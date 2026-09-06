# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-4234948b4607744a391b5ec2cbe14a9b10504c1f4f981a20a52284cf13a2e193"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP release readiness reports now also promote the Ethereum Core message
  replay source inventory to a production gate, so durable pinned-record replay
  protection and negative replay/history tests must stay pinned before active
  Ethereum launch evidence can pass. Readiness and strict-bundle sparse tests
  must remove every Core implementation, negative replay/history, readiness,
  and strict-bundle marker directly, and the strict-bundle verifier inventory
  must pin its own replay sparse guard so marker-level coverage cannot be
  dropped while the inventory row remains present.


<a id="record-d910da0c42b99431f3d62b6e9c5f6891d2f5825747c12953418780b6f1b78519"></a>

- SCCP release readiness reports now also promote the Ethereum Torii pinned
  message-proof source inventory to a production gate, so public readback keeps
  serving only pinned bridge records and negative unpinned-record serving tests
  remain pinned before active Ethereum launch evidence can pass. Readiness and
  strict-bundle sparse tests must remove every Torii routing, readiness, and
  bundle marker directly, and the strict-bundle verifier inventory must pin its
  own Torii sparse guard so public readback coverage cannot be dropped while the
  inventory row remains present.


<a id="record-b9f44323183b6f363cb8dbf3ebeee0be79f2dcd93d2a9f6642c84ed6d39af2df"></a>

- SCCP release readiness reports now also promote the active Ethereum EVM live
  source and destination evidence inventories to production gates, so canonical
  live RPC chain ids, finalized block tags, deployment receipt binding, runtime
  bytecode hashes, route canary calldata, and proof tuple drift regressions must
  stay pinned before active Ethereum launch evidence can pass. Readiness and
  strict-bundle sparse tests must remove every active Ethereum source-live
  marker directly across the source collector, adversarial source-live tests,
  copied all-lanes runtime-bytecode redaction coverage, readiness wiring, and
  bundle wiring, and the strict-bundle verifier inventory must pin its own
  source-live sparse guard so marker-level live-source coverage cannot disappear
  while the inventory row remains present. Destination-live readiness and
  strict-bundle sparse tests must do the same for live destination collection,
  route-canary calldata/proof validation, copied runtime-bytecode TOML
  redaction, readiness wiring, and bundle wiring, with the strict-bundle guard
  itself pinned in the inventory.


<a id="record-2cf367871ad6189af6c7ccd0b567dd45ec59134ab0eff98d9a555ca48154ddc1"></a>

- SCCP release readiness reports now also promote the Ethereum launch-policy
  selector source inventory to a production gate, so the `EthereumMainnetLane`
  selector and negative cross-lane policy regressions must stay pinned before
  active Ethereum launch evidence can pass. Readiness and strict-bundle sparse
  tests must remove every selector marker across Rust launch-policy logic,
  negative cross-lane regressions, readiness wiring, readiness tests, and bundle
  wiring, with the strict-bundle sparse guard itself pinned in the inventory.


<a id="record-a5a61d97107a0da09d4e5a645cc63516f5b6add50e85c7e4a17012466337b09c"></a>

- SCCP release readiness reports now also promote the Ethereum route-canary
  finalized receipt-block source inventory to a production gate, so finalized
  receipt-block binding, route-canary TOML fields, all-lanes comments, runtime
  hashing, and negative drift tests must stay pinned before active Ethereum
  launch evidence can pass. Readiness and strict-bundle sparse tests must remove
  every finalized receipt-block marker across EVM live collection, destination
  TOML generation, all-lanes metadata, Rust evidence hashing/config admission,
  readiness wiring, and bundle wiring, and the strict-bundle verifier inventory
  must pin its own route-canary sparse guard.


<a id="record-e542326b4fba89e27b3c0965b73267dcfdf5e2a26a442219eea1d0908fc86bdb"></a>

- SCCP release readiness reports now also promote the active Ethereum EVM
  block-tag metadata source inventory to a production gate, so finalized source
  and destination block-tag evidence and negative drift tests must stay pinned
  before active Ethereum launch evidence can pass. Readiness and strict-bundle
  sparse tests must remove every block-tag marker across EVM source/destination
  collectors, ETH/BSC source/destination TOML helpers, all-lanes metadata
  preflights, adversarial tests, readiness wiring, and bundle wiring, and the
  strict-bundle verifier inventory must pin its own block-tag sparse guard.


<a id="record-ac351d8adcb4769dfb4fba5c6a7bd5c64a9360d29189be0505c6c214ed0aff33"></a>

- SCCP corridor phase evidence must also stay source-unique: downloaded
  `--phase-evidence-dir` logs and explicit `--phase-evidence` assignments
  cannot set the same phase twice, so release reports and bundles cannot
  silently replace one hashed phase transcript with another. `--phase-result`
  and `--phase-evidence` phase names, plus `--phase-result` status values,
  must also reject padded or whitespace spellings instead of trim-normalizing
  them into canonical corridor phases or statuses, and phase names must reject
  Markdown-unsafe or malformed values before diagnostics can echo them. Unknown
  phase names and unknown phase-result statuses must use category-only
  diagnostics instead of echoing operator-supplied Markdown-unsafe text, and
  duplicate phase-evidence diagnostics must redact local evidence paths as
  `<path>`. Missing
  `--phase-evidence-dir` logs must report the standard checked layouts without
  echoing the operator-supplied directory.


<a id="record-822f865e36d1c9033aa7260b5310b41b5e9073a4244365e430ba269f7a25fc60"></a>

- Active BSC mainnet SCCP SDK hardening now directly gates malformed
  receipt-observed source-event logs: browser, Python, Swift, Kotlin/JVM, Java
  Android, and .NET tests reject matching BSC source-bridge logs with extra
  topics, non-empty data, zero digests, duplicate/removed events, or missing
  transaction context, and the release bundle verifier requires those markers
  before the BSC lane can be advertised as ready.


<a id="record-66f51f0b91e167aa17ae64bdae307f559f4c841955ca1d0b72a1ebf2f35a5b1e"></a>

- Rust SCCP canonical transcript packaging now uses checked `u32`
  length-prefix writers on production `Option` admission paths, so oversized
  Merkle-proof, bundle, finality-proof, transparent-statement, and
  source-chain proof-envelope transcript fields fail closed before TON,
  native/local, platform, TAIRA diagnostic packaging, or runtime finality
  export. SCCP source-adapter verification statement, adapter-commitment, and
  FastPQ context packaging also reject unbounded adapter-proof shapes and
  oversized checked length prefixes before proof batch construction.


<a id="record-e1a5bf29af2eff3d3a2b16cff7535ff00093c78d81602ba9c067cfe396c34c5d"></a>

- Ethereum mainnet SCCP release gating now treats the published JS browser
  artifact as a first-class launch surface: the strict release bundle verifier
  scans both source and `dist` for receipt-proof admission guards that bind
  block receipts, receipt roots, execution headers, finalized Beacon roots,
  and sync committee roots before browser evidence can be advertised as
  production-ready. It also scans native SDK receipt-proof builders for
  block-receipt metadata binding and typed receipt rejection, and requires
  every outbound Ethereum SDK facade, including Python, to validate the
  configured mainnet execution provider before caller-supplied submission
  callbacks can run. The Rust source-adapter readiness gate now also checks
  ETH/BSC EVM deployment material explicitly, so Ethereum mainnet readiness
  rejects replayed source bridge network ids, config hashes, and emitters
  before proof packaging, with strict release-bundle markers pinning the gate
  and regression coverage. Core `SubmitBridgeProof` admission now also binds
  typed SCCP message proof ranges to artifact finality height, with an ETH
  local-admission range-replay regression and strict release-bundle markers;
  the same core path requires SCCP message proof records to be pinned, keeps
  pinned bridge proofs out of manual pruning, and rejects a second retained
  proof for the same `(source_domain, target_domain, messageId)` only when the
  retained record is verified, pinned, and internally consistent. Torii's
  SCCP message-bundle submission path now emits pinned bridge proofs as well,
  so app-facing `/v1/bridge/proofs/submit` payloads remain compatible with
  core replay protection; its proof-registry read side also refuses to serve
  unpinned SCCP message records as non-SORA source-chain envelopes.
  Live Ethereum evidence scripts and all-lanes imports now keep finalized
	  block-tag metadata under the same strict release-bundle verification, and
	  the diagnostic unready transparent-proof bypass surface is removed from
	  runtime config and case-variant, split-token, or source-escaped forms of
	  the old environment override are rejected by the release verifier.
  Production-ready BSC/TRON route-config renderers must reject any
  `--allow-unready` option and non-production route manifests, so governed
  runtime overlays cannot re-enable diagnostic transparent-proof admission
  while claiming production readiness.
  Command-level BSC/TRON route-config paths must leave pre-existing TOML
  artifacts untouched when malformed `--allow-unready` values are supplied.
  BSC route-config full-config evidence generation must reject
  `--write-offline-full-toml-evidence` path collisions with `--out`,
  `--manifest`, or `--base-config` before replacing route TOML or input
  artifacts.
  BSC/TRON route-config commands must also reject `--out` paths that resolve to
  `--manifest` or `--base-config`, so generated overlays cannot replace their
  own input manifest or deployed base config; explicit `--out` collisions must
  fail this boundary before parsing copied manifest or base-config input.
  BSC native-prover bundle attachment must reject `--out` collisions with
  `--attach-route-manifest-out` and the source route/deployment evidence, so
  bundle JSON cannot be overwritten by an attached manifest or replace its
  source binding evidence.
  BSC publish-route-manifest and publish-burn-record-vk commands must reject
  `--out` collisions with their route manifest or VK-template inputs before
  producing ISI artifacts, including symlink-parent aliases that resolve back
  to those reviewed inputs. BSC route-manifest publication must also preserve
  raw Torii pipeline HTTP rejections, including bounded public response
  previews, before any JSON fallback that depends on a local native decoder.
  Release-readiness and strict release-bundle source inventory must pin the
  direct, merged, removed-option, stale-key stripping, and non-production
  route-config rejection tests, plus the absence of the deleted
  `sccp_allow_unready_transparent_proofs` config/schema surface, before this
  gate can pass. Readiness and strict-bundle sparse tests must
	  remove every uniquely detectable unready transparent-proof config marker from
	  each inventory row, and must separately assert the forbidden environment
	  override scan, including lowercase, mixed-case, source-literal-split, or
	  source-escaped override names, so this gate cannot degrade to a few hand-picked BSC/TRON
  route-config assertions.
  Strict release-bundle verifier inventory now also pins the release-bundle
  unready-config guard and missing-gate regressions directly, so those bundle
  tests cannot disappear while readiness-report inventory still passes.
	  BSC deployment helper booleans must also stay exact: malformed, padded,
	  uppercase, alias, empty, null, object-wrapper, boolean, or numeric values
	  for `--broadcast`, `--confirm-mainnet`, and `--allow-local-rpc` must fail
	  before signer, RPC, or network operations can proceed.
	  `--allow-diagnostic-verifier` is removed and must fail before verifier
	  reads, while diagnostic BSC verifier material is always refused before
	  signer or RPC lookup. The shared BSC boolean parser now keeps missing
	  options as false/fallback while rejecting explicitly supplied malformed
	  values, and the route-config `--allow-unready` regression uses the same
	  hostile corpus.
	  BSC route-manifest and burn-record VK publication booleans must keep the
	  same exact boundary: malformed `--submit` and `--wait-for-commit` values
	  must fail before local ISI artifact replacement, transaction signing, native
	  transaction construction, or Torii fetches can proceed. Submit-only metadata
	  such as authority and gas limit must fail at the same pre-write boundary,
	  so malformed local submission parameters cannot replace reviewed ISI
	  artifacts before signing starts.
	  BSC route-manifest readiness booleans must also stay exact before artifact
	  replacement: malformed `--production-ready`, `--live-readback-checked`,
	  `--full-toml-ready`, or mainnet `--confirm-mainnet` values must leave any
	  pre-existing route-manifest output untouched.
  BSC route-manifest JSON ingestion must likewise keep readiness fields exact:
  `productionReady` and `postDeployLiveEvidence.fullTomlReady` must be JSON
  booleans, and string/numeric truthy values must be rejected by route-config
  tests plus release-readiness/strict-bundle source inventory.
  BSC deployment helper `--rpc-url`, publish `--torii-url`, and browser-prover
  HTTPS `moduleUrl` values must use exact URL text and public-DNS HTTPS, with
  loopback HTTP reserved for explicit local development paths; local browser
  module references must also be package-relative and must not traverse parent
  or root paths, so credentialed, params/query/fragment-bearing, localhost,
  IP-literal, single-label, `.local`, malformed DNS-label, padded,
  control-bearing, parent-traversing, root-relative, or encoded-escape endpoint
  or module values cannot reach RPC, Torii publication, or route-manifest
  browser module references. Rust route-manifest config parsing and on-chain ISI
  admission must enforce the same browser-module boundary, so root-relative,
  internal HTTPS, credentialed, params/query/fragment-bearing, or non-package
  local module references cannot be reintroduced after CLI generation.
  TON TAIRA XOR route-manifest CLI handling must also stay fail-closed:
  duplicate options are rejected with fixed redacted diagnostics before output
  is replaced, every non-help option must carry an explicit value, input and
  output path options must be non-empty and unpadded before filesystem work,
  unknown commands and unknown named options fail with fixed command-scoped diagnostics before
  artifacts or manifest reads, command-specific help cannot hide unknown options
  or carry values, unexpected positional arguments are redacted rather than
  echoed, and `ton_finalize_message_value_nano` remains an exact, unpadded
  positive decimal JSON string on the publication path. Publish-route-manifest
  `--submit` and `--wait-for-commit` options must
  accept only exact `true` or `false` values before any artifact is replaced or
  signing path is reached. `--vk-name` and copied
  `taira_burn_record_vk_name` values must be exact verifier-key identifiers
  without padding, control text, or path-like separators, so operator input
  cannot silently trim into the governed burn-record key.
  `--private-key-env` must be a bounded uppercase
  environment variable name before runtime secret lookup. Publish `--torii-url`
  values must use public-DNS HTTPS unless loopback HTTP and must not carry
  credentials, query strings, fragments, internal HTTPS hosts, or
  whitespace/control-bearing URL text before
  submission work. Top-level `explorer_url` metadata and post-deploy
  source-event/route-canary explorer URLs must be public DNS HTTPS URLs without
  credentials, query strings, fragments, localhost/IP-literal hosts,
  single-label internal hosts, padding, or control text before route-manifest
  artifacts can copy them, and copied top-level `explorer_host` must match the
  normalized `explorer_url` host. Browser-prover HTTPS
  `module_url` values copied from destination/source prover manifests must use
  the same public DNS host policy; package-relative module references and
  loopback HTTP remain the only non-public development forms, and root-relative
  or parent-traversing local module references must be rejected before
  route-manifest artifacts can copy them. Publish
  `--authority` values must be canonical I105 account
  ids before runtime secret lookup, so alias, hex/UAID, padded, control-bearing,
  or secret-looking authority text cannot reach signing. Submit metadata such as
  `--chain-id`, `--torii-url`, and `--commit-timeout-ms` must be rejected before
  runtime private-key lookup. Review-only publish artifacts must reject
  submit-only flags unless `--submit true`, so authority, private-key-env,
  Torii URL, chain-id, wait-for-commit, or commit-timeout settings cannot be
  silently ignored. Publish review gas metadata must validate before manifest
  reads, so invalid gas asset ids or gas limits cannot be masked by missing or
  malformed route manifests. Route-manifest `--out` paths must also stay
  distinct from all input evidence paths, and
  publish-route-manifest `--out` must stay distinct from the manifest being
  published, including through symlink-parent aliases, before any reviewed
  evidence can be replaced.
  Release-readiness and strict-bundle source inventories must pin those
  Node-script regressions.
  TRON deployment helper operator booleans must stay exact as well: malformed,
  padded, uppercase, alias, boolean-object, or numeric values for
  `--broadcast`, `--force`, `--check-account`, `--require-secret`,
  `--require-verifier`, `--require-optional-packages`, route-manifest
	  `--production-ready`, and route-manifest `--live-readback-checked` must fail
	  closed before deployer rotation, doctor prerequisite, account-check,
	  readiness acknowledgement, live-evidence, route-manifest artifact replacement,
	  or broadcast paths can proceed;
	  the deploy `--broadcast` path now uses the same shared exact-boolean parser
	  and the regression corpus covers empty, null, object-wrapper, padded,
	  uppercase, alias, boolean, and numeric values across deploy, doctor,
	  route-config, and route-manifest switches;
	  `--endpoint` overrides must also be exact HTTPS URLs with public DNS hosts,
	  so credentialed, params/query/fragment-bearing, localhost/private/IP-literal,
	  single-label, `.local`, malformed DNS-label, padded, or control-bearing
	  endpoint values fail before doctor, account-status, deploy, broadcast, or
	  polling requests can be built;
		  release-readiness and strict bundle source inventories must pin those
		  adversarial regressions. Their sparse tests must remove every TRON deploy
	  operator boolean marker, including release-gate wiring markers, so the gate
	  cannot degrade to a few hand-picked deploy-script assertions. Required
	  Release Evidence must also carry the SCCP TRON deploy operator boolean
	  source-inventory row before public bundle readiness can pass.
	  Strict release-bundle verifier inventory now also pins the bundle-level
	  TRON deploy operator boolean inventory regression before public readiness.
  TRON route-manifest JSON ingestion must also reject non-boolean readiness
  state: `productionReady`, `postDeployReadbackChecked`, and supplied
  `postDeployLiveEvidence.fullTomlReady` cannot be string/numeric truthy
  values, and the route-config tests plus release-readiness/strict-bundle
  source inventory must pin those cases.

