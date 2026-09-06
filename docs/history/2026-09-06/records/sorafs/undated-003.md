# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-3588582588bf5c839595c6c4e5e0213700930f2475bab209b508b6c7a363cbf9"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SoraFS gateway fixture version `1.0.0` now includes a detached Ed25519
  council envelope generated from the deterministic fixture-only council key.
  `xtask sorafs-gateway-fixtures --verify` validates the envelope JSON shape,
  signer public key, manifest digest, chunk-plan digest, profile aliases, and
  signature, and the published fixture metadata now pins the envelope digest in
  the aggregate bundle hash. The prior fixture-envelope placeholder is closed;
  future work should replace or add release governance key material only through
  the normal signed release process, not by reintroducing placeholder
  signatures.


<a id="record-9c1fa15562f825e10b07fcd3ab23e7c22e0dd165eb91209bfb61bd0bb835884f"></a>

- SoraFS SF-11 reference validator now has provider-advert,
  provider-admission-envelope, replication-order, orderbook payload, PoR
  challenge/proof, PDP commitment/challenge/proof, PoTR receipt, repair
  payload, fixture-directory bundle, governance log node, governance DAG block,
  and signed governance DAG head-chain implementation slices:
  `sorafs_manifest` exposes
  `ValidationOutcomeV1`, `validate_provider_advert_bytes`,
  `validate_provider_admission_envelope_bytes`,
  `validate_provider_admission_renewal_bytes`,
  `validate_provider_admission_revocation_bytes`,
  `validate_replication_order_bytes`, `validate_orderbook_payload_bytes`,
  `validate_pdp_commitment_bytes`, `validate_pdp_challenge_bytes`,
  `validate_pdp_proof_bytes`, `validate_pdp_commitment_challenge_bytes`,
  `validate_pdp_challenge_proof_bytes`,
  `validate_pdp_commitment_challenge_proof_bytes`,
  `validate_por_challenge_proof_bytes`, `validate_potr_receipt_bytes`,
  `validate_repair_payload_bytes`, `validate_fixture_bundle_payloads`,
  `validate_governance_log_node_bytes`,
  `validate_governance_dag_block_bytes`,
  `validate_governance_dag_head_chain_bytes`,
  `validate_signed_replication_order_bytes`,
  reusable provider-advert Ed25519 signature verification, signed
  replication-order Ed25519 signature verification, and the `reference_ffi` C
  ABI facade returning
  `ValidationOutcomeV1` Norito JSON buffers for signed replication-order,
  admission renewal/revocation, orderbook payload, PDP
  commitment/challenge/proof payload, and peer validators for SDK bindings.
  `crates/sorafs_manifest/include/sorafs_reference.h` now provides the checked C
  header for downstream bindings, and
  `ci/check_sorafs_reference_ffi_header.sh` rejects Rust/header export,
  signature, or selector drift while reading the Rust FFI source, C header, and
	  negative-control copies through no-follow descriptors with complete
	  byte-write loops plus descriptor fsync. The release packager
  stages that header under `include/`, records its SHA256 in the per-target
  manifest, writes
  metadata-normalized tar/gzip archives with sorted entries and fixed ownership,
  mode, and mtime, rejects symlinked archive-output parents, stage roots,
	  parent chains, and staged entries before archiving, writes archive bytes and
	  reads staged files through no-follow descriptors, fsyncs the completed
	  archive stream plus output parent directory, and writes the release manifest
	  JSON through a no-follow descriptor with strict bytes, a complete write loop,
	  descriptor fsync, and output parent-directory fsync. The packager also
	  rejects missing or option-shaped wrapper option values, symlinked or
	  parent-aliased prebuilt binaries, manifest signing keys, manifest public
	  keys, and checked FFI headers, and symlinked output directories before any
	  cleanup can remove staged artifacts. Its binary, archive,
	  and manifest SHA-256 sidecars now also use the no-follow complete-byte
	  writer with descriptor and output parent-directory fsync instead of shell
	  redirection. Optional detached manifest signatures are installed from an
	  OpenSSL temp file through the same no-follow complete-byte, descriptor
	  fsync, and parent-directory fsync discipline while rejecting symlinked
	  signature outputs and manifest overwrite attempts,
  and the local SoraFS release gate runs the header-contract
  guard and adversarial release-helper tests before Clippy/tests. The
  JavaScript SDK now exposes the Rust-backed
  orderbook and PDP reference validators from both the package root and
  `@iroha/iroha-js/sorafs`, while the Python SDK exposes the same
  `ValidationOutcomeV1` contract from `iroha_python.sorafs` and the package
  root for orderbook payloads, PDP single payloads, PDP commitment/challenge
  pairs, PDP challenge/proof pairs, and full PDP commitment/challenge/proof
  bundles. Kotlin/JVM, Java Android, and Swift expose matching source wrappers
  through the shared `connect_norito_bridge` native facade. It also ships the
  `sorafs-validate advert` /
  `sorafs-validate admission` / `sorafs-validate order` /
  `sorafs-validate orderbook` / `sorafs-validate por` /
  `sorafs-validate pdp` / `sorafs-validate potr` /
  `sorafs-validate repair` / `sorafs-validate bundle` /
  `sorafs-validate governance` / `sorafs-validate sign --kind advert` /
  `sorafs-validate sign --kind order` /
  `sorafs-validate sign --kind orderbook` /
  `sorafs-validate sign --kind governance` CLI commands. The CLI emits stable
  Norito JSON/table/YAML outcomes, returns code `2` for
  validation/policy/signature/Norito payload failures, and points `docs_url` at
  the portal error catalogue. The rollout-gate static contract now also pins
  the SF-10 proto plan's active fixture-generator/stub commands to binaries
  that exist in the current crates, so retired generator names cannot re-enter
  the required fixture-refresh workflow. The same static contract also pins
  live release-bundle publication, SDK-smoke publication, schema-registry or
  wire-format service promotion, and any separate `sora-proto` codec surface as
  unshipped with reusable route/CLI matchers and segment-aware negative
  controls. It now also scans CLI sources for nested deployed-only `sora
  proto`, `sorafs proto`, `proto schema-service`, `schema-registry service`,
  `schema registry service`, `wire-format service`, `wire format service`,
  `proto release-bundle`, `fixture release-bundle`, `fixture-bundle publish`,
  `sdk-smoke publish`, and `proto promote` spellings while preserving the
  Norito-only boundary, committed `.to` fixtures, JSON commentary,
  `ValidationOutcomeV1`, reference FFI validators, `sorafs-validate bundle`,
  active fixture generators, and payload-free proto canary/evidence labels.
  `provider_admission_fixtures` now writes binary, JSON, and README artifacts
  through checked no-follow descriptor outputs, uses canonical temp roots in
  tests, and keeps its digest regression aligned with the checked-in metadata.
  Admission validation covers base envelopes,
  governed renewals against their previous envelope digest, and governed
  revocations against the envelope digest and council signatures. Bundle
  validation checks known fixture-directory artifacts, validates discovered
  orderbook order/trade/channel/settlement fixtures, checks PoR and PDP
  challenge/proof binding, checks PDP commitment/challenge/proof binding, shared
  manifest digests for manifest-bearing artifacts, provider-admission provider
  consistency, and replication-order provider assignments. Governance
  validation checks
  `GovernanceLogNodeV1` structure, embedded payload policy, publisher metadata,
  signature material, Ed25519 and Dilithium3/ML-DSA publisher signatures, and
  required node-CID binding. Signed replication-order validation checks
  `SignedReplicationOrderV1` structure and verifies Ed25519 signatures over the
  `sorafs.replication_order.signature.v1` domain-separated canonical order
  signing bytes. Advert, order, and governance signing sign canonical payload
  bytes with runtime-supplied Ed25519 seeds, write Norito output only after
  validation succeeds, and emit the same reference outcome contract. Committed
  fixtures now include deterministic orderbook/streaming-settlement payloads
  plus runtime replay snapshot bytes, PDP commitment/challenge/proof payloads
  plus expanded negative fixtures, PoTR receipt, and repair task payloads under
  `fixtures/sorafs_manifest/orderbook/`, `fixtures/sorafs_manifest/pdp/`,
  `fixtures/sorafs_manifest/potr/`, and `fixtures/sorafs_manifest/repair/`, so
  orderbook and PDP fixture tests cover the committed bytes and bundle
  validation exercises orderbook runtime snapshots, PDP, PoTR receipt, and
  repair payloads directly from a clean checkout. PDP proof validation now
  rejects empty segment and hot-leaf Merkle paths, focused validator tests cover
  late proofs, wrong providers, wrong manifests, and witness coverage
  mismatches, and the committed fixture inventory now includes the expanded
  negative set with `.to`/JSON parity checks. The SF-13 rollout evidence gate now requires
  payload-free provider-transport, proof-generation, validator-replay,
  governance/repair, observability, and governed-approval artifacts before PDP
  promotion, and requires replay/governance/observability/approval artifacts to
  bind back to a valid proof-generation `proof_summary_digest_hex` in the same
  evidence bundle. The lane checker also has direct adversarial coverage that
  forges `proof_summary_digest_hex` on every proof-summary-bound downstream
  kind, so validator replay, governance/repair, observability, and governance
  approval evidence all fail against mismatched proof-generation summaries
  before promotion. It requires governance approval `policy_digest_hex` to match a
  valid proof-generation `policy_digest_hex`, publishes valid PDP policy
  digests as `valid_policy_digests`, requires governance approval
  `provider_roster_digest_hex` to match a valid proof-generation
  `provider_roster_digest_hex`, publishes valid governed provider-roster
  digests as `valid_provider_roster_digests`, with binding failures marked on the
  offending artifact through the shared scalar binding error recorder before
  required-kind summary validity is reported. The lane checker also has direct
  adversarial coverage for every policy-bound and provider-roster-bound PDP
  artifact kind, so governance approval evidence fails against detached
  proof-generation policy or provider-roster digests before promotion.
  Proof-generation artifacts now
  also bind `provider_count`, `challenge_count`, and `proof_count` to the unique
  canonical `providers[].name`, `challenges[].name`, and `proofs[].name`
  inventories, rejecting duplicate provider, challenge, or proof rows before
  PDP readiness can report ready, requires those provider inventory labels to
  use reviewed lowercase `provider-*` IDs without non-production markers, and
  requires challenge and proof inventory labels to use reviewed lowercase
  `pdp-challenge-*` and `pdp-proof-*` labels without non-production markers,
  binds provider-transport `route_count` to the
  reviewed route-name inventory with unknown route rejection, and rejects
  duplicate or unknown observability metric rows before metric evidence can
  report ready. Provider-transport route latency evidence must now be a
  non-negative integer before it can satisfy the route-latency ceiling, and
  proof-generation `max_proof_latency_ms` must be a positive integer before it
  can satisfy the proof-latency ceiling, so impossible negative or fractional
  timings cannot pass rollout thresholds. The PDP gate now also has
  missing-field regressions proving `response_bodies_included`, raw
  challenge/proof/export/report flags, and `critical_alerts_firing` must be
  explicitly encoded as `false`. PDP summaries also export the reviewed
  observability `metrics` inventory plus `metric_count_values`, and the
  aggregate production-readiness gate tethers both fields to observability
  artifact fingerprints before final promotion can report ready. Governance/
  repair artifacts now also fingerprint `repair_handoff_digest_hex`, PDP
  summaries export `valid_repair_handoff_digests`, and the aggregate
  production-readiness gate tethers those values to governance/repair artifact
  fingerprints before final promotion can report ready. The aggregate
  production-readiness gate now also rechecks proof-summary-bound,
  policy-bound, and provider-roster-bound artifact fingerprints against
  `valid_proof_summary_digests`, `valid_policy_digests`, and
  `valid_provider_roster_digests`, plus repair-handoff metadata against
  `valid_repair_handoff_digests`, before final promotion can report ready. The
  PDP collection planner now emits the
  checker-backed `evidence_contract` map for the selected required kinds during
  dry-run review, and validates the schema-closed collection-plan envelope plus
  canonical nested required-kind, threshold, external-evidence, checker-backed
  evidence-contract, and command-step shapes before dry-run output or verifier
  execution.
  `scripts/build_sorafs_pdp_canary.py` now builds payload-free checked-in
	  canary artifacts for each SF-13 gate kind, requires complete PDP route and
	  metric coverage where applicable, rejects duplicate or unknown route/metric
	  inputs before writing, binds provider-transport `route_count` to
	  the unique canonical route-name inventory so duplicate or unknown route rows
	  cannot inflate readiness, requires every provider route response to carry a
	  `body_blake3_hex` digest, enforces proof-summary digest bindings plus
	  proof-generation/governance policy-digest and provider-roster digest input,
	  reviewed provider/challenge/proof inventories whose unique names match their
	  scalar counts while rejecting duplicate or non-production provider,
	  challenge, or proof names, explicit `--route-body-blake3-hex` evidence for
	  provider routes, `--repair-handoff-digest-hex` evidence for governance/
	  repair handoff, and latency thresholds before writing, validates every generated
  artifact through the PDP rollout checker, and ships provider-transport and
  proof-generation response-file examples. The authenticated PDP challenge,
  next-work, proof-submission, status, and terminal-export routes are shipped
  locally, and embedded proof streaming admits `proof_kind=pdp` only for a
  supplied challenge id. That protocol-local completion does not close SF-13:
  proof failures now use a fail-closed exact-chain durable native repair
  transaction handoff and storage execution is gated by the exact finalized
  lease. Production still requires removing residual local repair projections,
  proving cross-peer exactly-once repair/finality reconciliation, deployed
  multi-provider signature/inclusion verification, Governance DAG archival,
  operator integration, and evidence that passes the gate. Static rollout
  guards must preserve the shipped authenticated provider routes while keeping
  only the remaining production service, authoritative repair, and promotion
  surfaces unshipped.
  `fixtures/documentation/sorafs_reference_sdk/` ships a runnable cookbook that validates
  committed fixtures, exercises advert/order/governance signing, checks
  orderbook receipt validation and bundle cross-links, and emits manifest/CAR
  replay outcomes for SDK and release smoke testing. The docs portal SoraFS
  packager now reads its generated package-summary rows through no-follow
  descriptors before emitting the final package summary, so developer-portal
  CAR/SBOM release metadata is not assembled from a symlinked summary input.
  The docs portal pin-release descriptor append path now also reads existing
	  descriptor JSON and writes updated strict descriptor bytes through no-follow
	  descriptors with complete byte-write loops plus descriptor fsync. The Rust `sorafs_cli` shared
  output opener now preflights output leaves and parent chains, creates missing
  output parents only after the chain passes inspection, opens generated
  summaries, manifests, response bodies, CAR archives, bytecode, storage
  payloads, fetch outputs, governance DAG archives, and proof/reputation
  summaries with platform no-follow final-component flags where available,
  rejects non-regular opened outputs, and writes bytes through the opened
  descriptor without unsafe caller-side parent pre-creation. `sorafs_manifest_builder` now applies that same checked
  descriptor contract to CAR archives, manifest bytes, JSON reports, hybrid
  envelope outputs, signature/public-key sidecars, and all capacity subcommand
  Norito/base64/JSON/request outputs, with capacity integration fixtures rooted
  under canonical temp directories so platform temp aliases do not trip the
  release-path guard. The `sorafs-node` CLI now applies the same no-follow
  descriptor contract to manifest, payload, plan, and PoR JSON outputs, with
  its CLI fixtures using canonical temp roots so platform temp-directory
  symlinks do not bypass the release-path check. Embedded `sorafs_node`
  storage index/manifest metadata persistence now validates output leaves and
  parent chains before creating parents, creates atomic temp files with
  create-new plus no-follow flags, rejects symlinked outputs and parents, and
  removes temp files after failed atomic writes. Embedded repair-store
  persistence now uses the same checked atomic contract for repair task,
  history, nonce, and audit-sequence snapshots, with canonical temp-root tests
  covering symlinked output rejection, symlinked parent rejection, preexisting
  temp symlink rejection, file-store reload, and manager-level persistence.
  Embedded governance DAG publisher persistence now applies that checked atomic
  contract to encoded governance payloads, JSON/digest sidecars, CAR queue
  metadata, runtime DAG blocks, and runtime DAG heads, with canonical
  temp-root coverage for publisher outputs and symlink/temp-file regressions.
  `sorafs_fetch` now uses the
  same checked descriptor opener for assembled payloads, streaming outputs,
  CAR archives, fetch reports, provider metrics, and chunk receipts, and its
  CLI integration plus in-binary CLI tempdirs are rooted under canonical temp
  paths so release output checks are not bypassed by platform temp-directory
  symlinks. The shared chunk-fetch plan parser now rejects noncanonical
  uppercase digest aliases, zero or oversized chunk lengths, byte-range
  overflow, chunk-index overflow, and non-object Taikai hint shapes before
  fetch/local/CLI consumers treat plan JSON as trusted. The SoraFS chunker now
  exposes fallible profile validation and checked chunk-end helpers, and CAR
  plan construction plus chunk-store ingestion use those fallible paths so
  malformed custom chunk profiles fail as structured errors instead of panic
  boundaries or silent `usize` to `u32` chunk-length truncation. The
  Soranet/CAR gateway manifest verification path now validates fetched
  manifests with council signatures required, and the broad SoraFS package
  validation fixtures use canonical temp roots so the no-follow release checks
  stay active on macOS temp-directory aliases. The
  `scripts/release_sorafs_cli.sh` signing wrapper, direct-mode smoke policy
  probe, and gateway telemetry probe now likewise read generated JSON summaries
  and policy/report inputs through no-follow descriptors before deriving hashes,
  persistence paths, or dashboard annotations. The rollout-gate contract now
  also scans SoraFS operator helpers to reject reintroduced plain
  `open`/`read_text`/`write_text`/`shutil.copy` paths and unreviewed recursive
  scans plus raw path resolution outside the shared path-identity boundary
  before those patterns can return to release tooling.
  `sorafs_car` now exposes `validate_manifest_car_replay` and
  `validate_manifest_car_replay_bytes`, and `soranet_trustless_verifier
  --validation-outcome` emits `ValidationOutcomeV1` for manifest policy plus
  CARv2 digest, root, chunk-plan, payload, and PoR replay. The streaming CAR
  verifier now also enforces exact CARv2 data-region coverage, rejects
  zero-length sections even when a tampered archive carries a matching manifest
  archive digest/size, and covers index bytes delivered in the same network
  update as the final data-region byte. Its summary and
  validation-outcome `--json-out` paths now also use the checked no-follow
  descriptor writer and canonical-temp integration coverage, so verifier
  evidence cannot be redirected through symlink leaves or parents.
  `da_reconstruct` reconstructed-payload and summary JSON outputs now use the
  same checked descriptor writer, and its RS parity reconstruction fixture has
  been refreshed to the current Norito DA manifest schema so the harness
  validates both live chunks and checked-in replay evidence. Taikai segment
  CAR/envelope/index/ingest-metadata outputs, `taikai_car` bundle summaries,
  multi-source fetch scoreboard persistence, and `taikai_viewer` metrics/summary
  artifacts now also use checked no-follow descriptor writers, with canonical
  temp roots in the affected CLI/integration coverage so platform temp symlinks
  cannot bypass the release-path checks.
  The SF-11
  release evidence gate now validates payload-free release-archive, signed-manifest,
  downstream-binding, cookbook-smoke, FFI/header-contract, and
  governance-approval evidence, requires release archive/downstream/cookbook/
  FFI/header/governance artifacts to bind back to a valid signed-manifest
  `release_manifest_digest_hex` in the same evidence bundle, with binding
  failures marked on the offending artifact through the shared scalar binding
  error recorder before required-kind summary validity is reported, requires
  signed-manifest evidence to carry `policy_digest_hex`, publishes valid
  signed-manifest policy digests as `valid_policy_digests`, publishes valid
  downstream `release_manifest_digest_hex` references as
  `valid_release_manifest_reference_digests`, requires the aggregate
  production-readiness gate to tether those reference digests to recognized
  artifact fingerprints, now also publishes archive-index, signed release-key
  fingerprint, package-index, smoke-output, header, and FFI-contract anchors,
  and requires the aggregate production-readiness gate to tether each one to
  its owning release evidence kind before reporting ready, and requires
  governance approval `policy_digest_hex` to bind back to that signed-manifest
  policy plus `public_key_fingerprint_hex` to bind back to that signed-manifest
  release key before SF-11 promotion can report ready; the
  matching collection planner accepts reviewed
  release evidence paths, supports
  `@ARGFILE`, forwards age, release-target, downstream-package, and
  smoke-duration thresholds, requires smoke duration to be positive
  integer-unit evidence before that ceiling can pass, and emits a dry-run-visible verifier command,
  selected-kind `evidence_contract`, and operator example args; it now validates
  the schema-closed collection-plan envelope plus canonical nested
  required-kind, threshold, external-evidence, checker-backed evidence-contract,
  and command-step shapes before dry-run output or verifier execution. The
  SF-11 plan
  now also ships
  `scripts/build_sorafs_reference_sdk_release_canary.py`, a fail-closed
	  payload-free release evidence builder for release archives, signed manifests,
	  downstream bindings, cookbook smoke, FFI/header contract, and governance
	  approval artifacts, with signed-manifest policy-digest inputs, governance
	  policy-digest and `--public-key-fingerprint-hex` inputs, and closed
	  release-target and downstream-package inventory validation that
	  rejects missing, duplicate, or unknown operator names before writing, along
	  with response-file examples for release-archive and signed-manifest
		  generation. The SF-11 plan
	  now also publishes the operator,
	  metrics, and binding-generation guides for packaging, telemetry extraction,
	  C FFI header synchronization, selector parity, and downstream package
  evidence handoff. The release evidence gate now also rejects duplicate or
  unknown release-target or downstream-package entries and requires `target_count` and
  `package_count` to match the unique canonical inventory lengths before
	  publication evidence can pass. Signed-manifest evidence and the
	  payload-free canary builder now also reject unsupported
	  `signature_algorithm`/`--signature-algorithm` values outside the governed
	  Ed25519 release algorithm (`ed25519`), including legacy RSA labels, before
	  those manifests can anchor release promotion, and the release summary now
	  exports `signature_algorithms` so the aggregate production-readiness gate
	  can tether that reviewed value to the signed-manifest artifact fingerprint
	  before final promotion, and aggregate promotion now also rechecks
	  manifest-bound and policy-bound artifact fingerprints against
	  `valid_release_manifest_digests` and `valid_policy_digests` plus governance
	  approval release-key fingerprints against `valid_release_key_fingerprints`.
	  The release gate now also requires exactly one active signed-manifest
	  digest, policy digest, and release-key fingerprint, clearing mixed
	  `valid_release_manifest_digests`, `valid_policy_digests`, or
	  `valid_release_key_fingerprints` before bound artifact or aggregate
	  metadata can promote.
	  The lane checker also has direct adversarial coverage that forges every
	  manifest-bound, policy-bound, and release-key-bound SF-11 artifact kind
	  against those signed-manifest anchors before release promotion can report
	  ready.
	  Remaining SF-11
	  work is per-target published archives, signed release manifests, downstream
  SDK package publication, and live operator smoke evidence that passes this
  gate, rather than local admission renewal/revocation, signing, governance
  publisher verification, reference cookbook, manifest/CAR replay coverage, or
  `sorafs-validate` packaging support: the packaging helper now records
	  staged-file and smoke output hashes in per-target manifests and can emit
	  detached manifest signatures when supplied governed release keys, with the
	  final signature installed through a no-follow complete-byte writer that
	  rejects symlinked outputs and manifest overwrite attempts. The
  rollout-gate static contract now also pins SF-11 per-target archive
  publication, signed manifest publication, downstream package publication,
  published cookbook/live-smoke evidence, and reference-SDK promotion routes or
  subcommands as unshipped with reusable route/CLI matchers and segment-aware
  negative controls. It now also scans CLI sources for nested deployed-only
  `reference-sdk publish`, `reference-sdk release-archives`,
  `reference-sdk signed-manifests`, `reference-sdk downstream-packages`,
  `reference-sdk live-smoke`, `reference-sdk published-cookbook-smoke`,
  `reference-sdk package-publication`, `sorafs-validate publish`,
  `sorafs-validate release-promote`, `published-archive smoke`,
  `downstream-bindings publish`, and `release-manifest publish` spellings while
  preserving the local `sorafs-validate` validator/signing commands,
  `scripts/package_sorafs_validate_release.sh`, FFI/header contract checks,
  cookbook fixtures, release evidence gate, collection planner, and
  payload-free canary/evidence labels. The
  rollout-gate static contract now also pins SF-6 CLI/SDK signed distribution,
  Homebrew/npm/crates.io/Go-module publication, live deployment capture,
  live-governance runbook capture, and release-promotion routes or subcommands
  as unshipped with reusable route/CLI matchers and segment-aware negative
  controls. It now also scans CLI sources for nested deployed-only `release
  distribute`, `release publish`, `distribution publish`,
  `homebrew|npm|crates publish`, `go module publish`, `sdk distribute`,
  `live governance capture`, `governance runbook capture`, and `release
  promote` spellings, while preserving the local `sorafs_cli` command families,
  `scripts/release_sorafs_cli.sh`, `ci/check_sorafs_cli_release.sh`, gateway
  self-cert tooling, SDK parity guards, release fixture smoke checks, and
  payload-free canary/evidence labels.


<a id="record-f1990d757795c26d9531bf8bf96a6ebcf968b4d20346d5e7004f3573d779f7ae"></a>

- SoraFS SF-9 PoR coordinator runtime integration is wired locally: Torii builds
  `PorCoordinatorRuntime` from `torii.sorafs_por`, starts it when the runtime and
  embedded storage are enabled, records scheduler challenge/forced/failure and
  duplicate-sample metrics through the existing telemetry handle, registers the
  PoR ingestion/scheduler metrics for Prometheus export, bounds PoR ingestion
  provider status readback with total/returned counts, retires the local
  unauthenticated PoR sampler, and bounds authenticated
  `/v1/sorafs/proof/stream` PoR requests to `sample_count=1..500` before
  finalized pin lookup and manifest sampling. It also adds PoR scheduler panels
  plus alert fixtures. The SF-9 rollout evidence gate now
  validates payload-free randomness, scheduler runtime, validator replay,
  reporting/archive handoff, exact SQL/Parquet archive-backend selection,
  governance archive handoff digest evidence, scheduler-runtime and
  reporting/archive `route_count` binding to the unique canonical
  `routes[].name` inventories with duplicate or unknown route rejection,
  randomness `provider_count` and `challenge_count` binding to the unique
  canonical `providers[].name` and `challenges[].name` inventories with
  duplicate provider/challenge rejection, reviewed lowercase `provider-*`
  provider labels without non-production markers, and reviewed lowercase
  `por-challenge-*` challenge labels without non-production markers,
  observability with reviewed metric-set coverage and duplicate or unknown
  metric-row rejection, and
  governance approval evidence, and requires scheduler/replay/reporting/
  observability/governance artifacts to bind back to a valid randomness
  `seed_replay_digest_hex` in the same evidence bundle, requires governance
  approval `policy_digest_hex` to match a valid randomness `policy_digest_hex`,
  publishes valid PoR policy digests as `valid_policy_digests`, with binding
  failures marked on the offending artifact through the shared scalar binding
  error recorder before required-kind summary validity is reported;
  the matching collection planner accepts reviewed staged evidence paths,
  supports `@ARGFILE`, forwards age, route-latency, scheduler-lag,
  report-latency, provider-count, and challenge-count thresholds, and emits a
  dry-run-visible verifier command, checker-backed `evidence_contract` map for
  the selected required kinds, plus operator example args; the gate now
  requires route latency, scheduler lag, and report latency to be
  non-negative integer-unit evidence before those thresholds can pass. The PoR
  gate now also has missing-field regressions proving raw
  randomness/VRF/challenge/proof/report/export flags, `response_bodies_included`,
  and `critical_alerts_firing` must be explicitly encoded as `false`. It now validates the
  schema-closed collection-plan envelope plus canonical nested required-kind,
  threshold, external-evidence, checker-backed evidence-contract, and
  command-step shapes before dry-run output or verifier execution, and
  `scripts/build_sorafs_por_canary.py` builds payload-free randomness,
  scheduler-runtime, validator-replay, reporting/archive, observability, and
	  governance-approval canary artifacts with randomness/governance policy-digest
	  input, reviewed randomness provider/challenge inventories, and closed runtime/reporting
	  route plus metric inventories through the same checker before rollout review,
	  with duplicate or malformed challenge inputs, duplicate or unknown route/metric
	  inputs, missing `--route-body-blake3-hex` evidence, and missing or malformed
	  `--governance-archive-handoff-digest-hex` evidence rejected before
  canary writes. The PoR coordinator, bounded local readback, and
  status/export/report protocol are locally implemented; proof failures use a
  fail-closed exact-chain durable native repair transaction handoff and
  storage execution requires the exact finalized lease. SF-9 is therefore not
  evidence-only: it still requires removal of residual local repair
  projections and cross-peer exactly-once finality reconciliation in addition
  to live
  drand/VRF/auditor run evidence and any operator-required governance archive
  handoff carried as
  `governance_archive_handoff_digest_hex` beside the deployment-specific
  SQL/Parquet `archive_backend` in reporting/archive artifacts, with the
  aggregate production-readiness `archive_backends` and
  `valid_governance_archive_handoff_digests` metadata tethered to those
  reporting/archive fingerprints before final promotion can pass. Aggregate
  promotion also rechecks seed-replay-bound and policy-bound artifact
  fingerprints against `valid_seed_replay_digests` and `valid_policy_digests`,
  and the lane checker has direct adversarial coverage that forges
  `seed_replay_digest_hex` on every seed-replay-bound downstream kind, so
  scheduler, validator, reporting/archive, observability, and governance
  evidence all fail against mismatched randomness before promotion. The local
  Torii coordinator, status/export/report endpoints, bounded ingestion
  readback, reference PoR validator, and scheduler observability are
  protocol-local foundations; they do not substitute for authoritative repair
  convergence. The
  rollout-gate static contract now also pins live external drand/VRF/auditor
  feed deployment, production archive/warehouse handoff, proof-bundle
  inspection, and SF-9 promotion routes or subcommands as unshipped with
  reusable matchers and segment-aware negative controls. It now also scans CLI
  sources for nested deployed-only `por live-deployment`,
  `por external-drand`, `por drand-feed`, `por vrf-feed`, `por auditor-feed`,
  `por production-archive`, `por archive-handoff`, `por proof-bundle
  fetch|show|replay`, and `por promote` spellings while preserving the local
  status/export/report/ingestion routes, capacity PoR proof/verdict routes,
  storage PoR sampling, `sorafs_cli por`
  commands, `sorafs-validate por`, canary evidence labels, and scheduler
  observability.

