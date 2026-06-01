# Roadmap

Last updated: 2026-06-01

This roadmap is the public, high-level view of current Hyperledger Iroha work.
The detailed engineering backlog lives in
[`docs/source/engineering_backlog.md`](./docs/source/engineering_backlog.md),
and completed history lives in [`status.md`](./status.md).

## Release and Stabilization

**Status:** active.

- Move the shared Iroha 2 / Iroha 3 codebase toward a broadly consumable
  release with clear release notes, SDK parity, and operator documentation.
- Keep focused validation green for the core transaction pipeline, Torii query
  and control-plane APIs, Norito wire formats, and SDK fixtures before broader
  workspace test runs. Torii app-query pages now expose concrete OpenAPI page
  schemas and SDK parsers for bounded `has_more`/`count_mode` metadata across
  the account, domain, asset, NFT, RWA, asset-holder, and repo-agreement
  list/query surfaces, and `torii_hot_paths` now includes sustained concurrent
  HTTP handler-path profiles for signed stored-cursor continuations, account
  alias projections, account-asset predicates, asset holders, committed-history
  contract activity, and generic aggregates, plus localhost socket profiles for
  the same workload set.
- Keep hardening the ISO 20022 bridge after the new inbound lifecycle endpoints
  and durable outbox helpers for `pacs.002`, `pacs.004`, `camt.029`, `camt.056`,
  `sese.023`, `sese.024`, and `sese.025`; remaining TradFi work is tracked in
  the engineering backlog for broader XMLDSig/XAdES trust-anchor and
  canonicalization fixture coverage, plus official MDR/XSD fixtures.
- Keep UI-side SCCP proof-generation SDK inputs fail-closed for ambiguous
  aliases; the current TON shard-state source-state path rejects duplicate
  camelCase/snake_case names inside nested validator-set transition proofs,
  including the transition-signature hash committed into the transition-chain
  witness.
- Keep public SCCP release evidence tied to every UI-side full-light-client
  role helper, not only aggregate request builders; Solana and TON readiness
  rows now require the per-role audit proof request symbols across web, Python,
  Swift, Kotlin/JVM, and Java Android.
- Keep the web portal SCCP proof-generation surface aligned with package
  artifacts; release-readiness tests now require every JavaScript/web helper
  named in the public user-prover rows to exist in source, packaged `dist`,
  package entrypoints, and TypeScript declarations, and strict release evidence
  plus published release-bundle verification must include the package-root SCCP
  export test transcript.
- Keep web SCCP linked-prover callback snapshots immutable across production
  destinations; JavaScript callback regressions now assert frozen request
  metadata where exposed and copy-backed bundle/source-proof bytes across TON,
  EVM-family, TRON, and Substrate-family proof engines before app-linked
  callbacks return proof bytes.
- Keep public SCCP phase evidence bound to executed production-corridor
  commands; release-readiness and release-bundle checks now require expected
  phase command fragments to appear on traced `+ ...` command lines inside the
  claimed phase block, not merely in incidental test output. The public bundle
  verifier also rejects prefix-alias phase markers, completion sentinels copied
  from a different phase block, and success markers that appear only on traced
  command lines instead of phase output. The verifier owns its required phase
  and phase transcript inventories independently of the report generator, with
  parity tests preventing drift.
- Keep Python SCCP package-root exports aligned with the public user-prover
  rows; release-readiness tests now import `iroha_torii_client` and require
  every non-callback Python helper/class to be exposed through `__all__`.
- Keep Python UI witness-provider inputs isolated from app-owned mutable
  objects; the SDK snapshot path now clones accepted non-string sequence inputs
  before user-provided witness resolvers run, so portal/mobile witness
  preparation cannot mutate the original proof request that the UI displays.
- Keep public SCCP user-prover helper rows one-to-one with real UI hooks;
  release-readiness tests and the release-bundle verifier now reject duplicate
  helper symbols in default and per-SDK rows so repeated names cannot stand in
  for omitted proof-generation entrypoints. The public bundle verifier owns the
  SDK phase inventory independently of the report generator, with parity tests
  preventing drift.
- Keep public SCCP user-prover rows tied to UI-owned proof hooks, not only
  request builders; readiness evidence and strict bundle verification now name
  the web/Python witness and prove callbacks, Swift witness/prove typealiases,
  Kotlin proof engines, Java Android nested proof engines, and Solana/TON
  source-state audit engines. The public bundle verifier now also owns the
  lane/SDK helper inventory for those proof-generation and on-chain submission
  entrypoints, plus the exact expected row construction and submission text, so
  weakening the report generator cannot remove cryptographic prover helpers or
  define a shorter portal/mobile table as canonical for published rows.
- Keep public SCCP user-prover rows gated by the real release phases; strict
  bundle verification now rejects duplicate, unknown, or missing required
  phases, requires every SDK plus core-admission on each row, and keeps
  EVM/TRON proof backends tied to contract-smoke evidence.
- Keep the public SCCP user-prover lane inventory fixed to production
  lane/backend pairs; strict bundle verification now rejects duplicate,
  unknown, or missing rows and backend-id drift for EVM/BSC, TRON, Solana, TON,
  and Substrate submission surfaces.
- Keep the public SCCP cryptographic evidence inventory fixed to production
  domains; strict bundle verification now rejects duplicate, unknown, or
  missing domain rows plus chain-label drift before comparing rows with
  embedded all-lanes evidence.
- Keep public SCCP cryptographic evidence rows tied to domain-specific route
  canary and source-gate policy; strict bundle verification now rejects
  incorrect canary sources, impossible source-gate requirements, and missing or
  unexpected named source-gate audit hashes.
- Keep public SCCP readiness Markdown verifier-owned and reviewer-complete;
  strict bundle verification now owns the canonical Markdown renderer, parses
  the Markdown sections independently, and requires copied evidence hashes,
  corridor artifacts, checklist statuses, cryptographic evidence rows,
  portal/mobile helper symbols, lane readiness rows, blockers, and
  release-evidence handoff text to appear in the public report.
- Keep public SCCP bundle verification free of generator backdoors for owned
  release artifacts; the verifier no longer exposes report/bundle module hooks
  for canonical Markdown, release-note attachments, copied-evidence summary
  recomputation, corridor inventories, crypto rows, or user-prover surfaces.
- Keep public SCCP release bundles rooted in immutable extracted directories;
  strict bundle verification now rejects a symlinked bundle root or a
  non-directory verifier input before reading the manifest.
- Keep public SCCP release manifests as verifier roots, not published artifacts;
  strict bundle verification now rejects any `manifest.json` row inside the
  manifest artifact table.
- Keep public SCCP release bundles free of unreviewed filesystem entries; strict
  bundle verification now rejects empty or otherwise unmanifested directories
  instead of comparing only files.
- Keep public SCCP release artifact paths printable and reviewer-safe; the
  readiness report, bundle builder, and strict verifier now reject ASCII control
  characters in manifest, report, and extracted bundle entry paths before they
  can reach Markdown tables or diagnostics.
- Keep public SCCP release evidence UTF-8 fail-closed; strict verification now
  reports non-UTF-8 manifest JSON, readiness JSON, all-lanes summary JSON,
  readiness Markdown, and release-note attachments as structured bundle
  failures instead of raising out of the verifier.
- Keep extending the Sumeragi formal corridor with independent TLC
  cross-checks; the current local TLC slice covers fast canonical frontier
  recovery, small exhaustive frontier recovery,
  validation redrive labels, raw QC signer-bitmap population counting, and
  signer-index normalization, precommit vote-progress counting, commit-QC
  signer quorum gating, commit-QC cache/history lookup, precommit signer record
  admission, validation ownership cleanup, stable worker-loop stage helpers,
  worker tick-gap scheduling, vNext performance config conversion,
  pending-block validation worker config derivation, commit-worker channel
  capacity normalization, slow commit-stage timing threshold detection,
  commit-inflight timeout reporting, post-commit pacemaker kickstart gating,
  idle-view proposal budget preservation, cached-slot timeout selection,
  pending fast-path timeout derivation, stalled pending-block timeout
  decisions, stalled pending-frontier timeout derivation, exact-frontier
  proposal grace derivation, exact-frontier slot helper semantics,
  exact-frontier slot tracker FSM behavior, slot tracker state map semantics,
  timeout/cooldown derivation semantics, round/view helper semantics,
  PhaseTracker mutable state semantics, failed-commit/block-sync helper
  semantics, missing-QC timing derivation, idle backlog signal derivation,
  proposal-liveness state transitions, actionable vote-backed proposal
  evidence admission, slot proposal evidence lookup and fall-through,
  round-liveness evidence aggregation, roster-unavailability recovery FSM
  transitions, consensus-recovery clear/prune retention semantics,
  frontier live-owner work preservation semantics, keep-frontier pending-active
  preservation semantics, stale-view pending prune cleanup semantics,
  superseded frontier payload retention semantics, stale missing-block request
  prune semantics, stale missing commit-QC request prune semantics,
  stale RBC session prune semantics, highest-QC defer-marker prune semantics,
  fast-finality inline validation semantics,
  observer signature-mismatch recovery semantics,
  validation failure finalization semantics,
  validation reject reason-label classification semantics,
  validation reject status accounting semantics,
  peer-key policy status accounting semantics,
  view-change cause status accounting semantics,
  view-change proof status accounting semantics,
  QC status projection semantics,
  commit-quorum status projection semantics,
  commit-inflight status projection semantics,
  history status projection semantics,
  RBC abort status accounting semantics,
  RBC mismatch status accounting semantics,
  RBC progress-stage synchronization semantics,
  RBC hot-repair/backpressure semantics,
  RBC repair request cooldown/targeting semantics,
  RBC targeted READY/DELIVER repair semantics,
  RBC outbound chunk flush semantics,
  RBC chunk post scheduling/debug-mask semantics,
  RBC READY/DELIVER deferral throttle semantics,
  RBC missing-INIT broad rebroadcast semantics,
  RBC persisted chunk sampling/proof semantics,
  RBC persisted session-store guard semantics,
  RBC store status accounting semantics,
  RBC store pressure-log throttling semantics,
  round-gap marker/snapshot/EMA status semantics,
  RBC stale-message/payload-refetch helper semantics,
  RBC missing BlockCreated recovery semantics,
  RBC unverified-roster escape-hatch semantics,
  RBC signing-preimage binding semantics,
  classic Vote/VRF signing-preimage binding semantics,
  classic Vote/QC signature-verification semantics,
  invalid-signature telemetry label semantics,
  invalid-signature throttle/penalty semantics,
  penalty offender-selection attribution semantics,
  consensus penalty-action derivation/application semantics,
  penalty status projection semantics,
  local peer removed flag semantics,
  execution-witness root projection semantics,
  RBC compact block-message semantics,
  consensus block-message priority semantics,
  block-message height/view projection semantics,
  block-message log/status kind projection semantics,
  consensus message projection semantics,
  pipeline event emission semantics,
  cached block-message wire-frame semantics,
  BlockCreated frontier metadata wire/rebuild semantics,
  cached proposal rebroadcast semantics,
  exact-slot frontier recovery activity semantics,
  frontier reassembly activity semantics,
  frontier quorum-owner cleanup preservation semantics,
  contiguous-frontier sidecar retarget semantics,
  contiguous-frontier sidecar expected-hash semantics,
  contiguous-frontier payload-hint selection semantics,
  contiguous-frontier parent-QC hint retarget semantics,
  vote-verification worker config derivation,
  QC aggregate-verification worker config derivation,
  voting-roster support counting, plus collector
  retry/gossip plans.
- The focused SCCP prover corridor is green for the current production-hardening
  slice across JavaScript, Python, Swift, Kotlin/JVM, Java Android, the Rust
  `iroha_sccp` verifier crate, core bridge-proof admission tests, and on-chain
  EVM/TRON Groth16 contract smoke coverage for post-generation payload,
  finality-height, and finality-block public-signal drift.
- EVM/BSC, TRON, Solana, TON, and Substrate user-prover readiness rows now
  include per-SDK helper symbol maps for JavaScript/web, Python, Swift,
  Kotlin/JVM, and Java Android. Those maps carry the native source-proof,
  source-state, full-light-client audit, or runtime-storage proof-generation
  helpers where applicable alongside the final proof request and submission
  helpers. Release bundles therefore cannot claim the portal/mobile native proof
  paths without explicitly carrying the UI proof-generation surfaces for each
  consumer SDK.
- Solana, TON, and Substrate native submission helpers now apply the same native
  recursive payload corridor to verifier-program/message-body/runtime-call
  `bundleBytes` as they already apply to proof bytes: bundles must be non-empty,
  non-all-zero, and no larger than 2 MiB before JavaScript, Python, Swift,
  Kotlin/JVM, or Java Android SDKs emit wallet/RPC instruction,
  internal-message, or runtime-call packages. The
  optional `sourceProofBytes` carried by SDK proof requests now share the same
  2 MiB source-proof corridor: omitted values remain valid, but non-empty
  source proofs must be non-all-zero and bounded before request hashing or
  app-linked user-prover invocation. The
  JavaScript and Python EVM-family/TRON contract-call submission builders now
  reject standalone `bundleBytes` or `sourceProofBytes` unless a wrapped
  `proofResult` is supplied, because raw Groth16 calldata cannot bind those
  request bytes back to the user-generated request hash. JavaScript and Python
  EVM-family, TRON, and Substrate-family submission builders now also reject
  explicit `proofResult: null` / `proof_result=None` instead of treating it as
  an omitted proof result, keeping null/omitted semantics aligned with Solana
  and TON before wallet or runtime-call packaging. Substrate-family submission
  builders across JavaScript, Python, Swift, Kotlin/JVM, and Java Android now
  also reject non-empty standalone `sourceProofBytes` unless a wrapped
  `proofResult` is supplied, because the final runtime-call payload carries the
  recursive bundle but not those request-bound source-proof bytes. The
  tracked JavaScript `dist/` package artifact is regenerated from that source
  and the package-dist suite now exercises the published `dist/index.js`
  Solana, TON, EVM/TRON, and Substrate submission guards, keeping the web
  portal SDK artifact aligned with the source guard. Public readiness reports
  now also require the
  JS corridor transcript to include the source SCCP tests, `package_dist`, and
  package export tests in the claimed `js-sdk` phase, so a release bundle cannot
  prove only source-side helper tests while omitting the dist artifact surface
  used by portal builds. The Rust `iroha_sccp` Solana, TON, and Substrate
  counterparty package builders now apply the same native recursive payload cap
  to canonical bundle bytes before emitting `SolanaProgramInstruction`,
  `TonInternalMessage`, or `SubstrateRuntimeCall` artifacts, keeping release
  tooling and portal/mobile SDKs on the same submission corridor.
- The all-lanes readiness and release-bundle verifier now derive a required
  `substrate_runtime_storage_gate_hash` for SORA-Kusama, SORA-Polkadot, and
  SORA2 from the governed Substrate source material plus source-adapter
  deployment records. Ready release bundles must carry that gate in the
  source-adapter audit hash set, giving Substrate-family runtime-storage source
  proofs the same machine-audited gate surface as the Solana, TON, and TRON
  source-adapter gates. The Substrate source-evidence renderer now also keeps
  JSON `toml_ready` false and refuses production TOML unless the governed
  runtime-storage gate hash is supplied and matches, so source material plus
  deployment pins alone cannot open the Substrate source lane. The all-lanes
  preflight now imports that same
  `sccp_substrate_runtime_storage_gate_hash` source-adapter audit comment and
  rejects missing, zero, or drifted Substrate runtime-storage gate metadata
  instead of treating a locally recomputed value as sufficient release evidence.
  Direct ETH/BSC source-evidence renderers now apply the same preimage rule to
  production TOML: hash-only source bridge code metadata remains diagnostic
  JSON, while `--toml` and JSON `toml_ready` require
  `--source-bridge-runtime-bytecode-hex` or
  `--source-bridge-runtime-bytecode-file` so the Keccak-256 runtime code hash is
  replayable from operator evidence.
- EVM route-canary evidence now uses a v2 transcript aligned with the TRON
  hardening model: ETH/BSC canary hashes bind submitted calldata SHA-256,
  decoded payload/finality public inputs, proof version/source domain, target
  domain, and consumed-message state before all-lanes preflight or Rust
  `iroha_sccp` route admission can mark route evidence launch-ready. TRON live
  evidence also requires source-event and
  route-canary transaction readback to contain exactly one matching governed log
  and rejects explicit `logIndex`/`log_index` metadata that disagrees with the
  log list position or supplies both aliases before production TOML can be
  emitted. TRON `gettransactioninfobyid` and `gettransactionbyid` source-event
  and route-canary readback now also reject conflicting `txID`/`txid`/`id`
  aliases before trusting receipt logs, raw-data hashes, or signature metadata.
  Raw transaction readback requires canonical `txID`, so an `id`-only response
  cannot be mistaken for a full transaction object. Source-event and
  route-canary transaction-info readback now require exact `blockNumber` and
  `blockTimeStamp` metadata, and source-event evidence cross-checks that
  timestamp against the fetched canonical block header. Saved source-event
  replay JSON and route-canary full-TOML replay now revalidate the same carried
  block metadata before producing offline arguments, so hand-edited summaries
  cannot bypass the live readback contract. Direct and live TRON full-lane TOML
  now also carry the route-canary block number and timestamp in audit comments
  plus structured route-allowlist fields, live replay forwards those values
  through the offline renderer before all-lanes readiness can pass, and
  release-bundle verification rejects missing, non-positive block numbers or
  negative timestamps before route evidence can be published. The public
  release-readiness cryptographic-evidence table now also carries the TRON
  route-canary block number and timestamp as verifier-bound JSON fields, while
  non-TRON lanes must keep those fields null, so release notes cannot publish a
  forged or lane-shifted canary height after refreshing attachment hashes.
  Source-event block transactions apply the same alias binding before deriving
  java-tron transaction Merkle leaves for source proofs. The EVM/BSC v2
  route-canary fields are also first-class config and ZK policy-hash material,
  keeping Core/Torii
  configured admission bound to the same calldata, payload, finality, and proof
  transcript that `iroha_sccp` validates. The full SCCP production corridor
  passes end to end with Rust SCCP verification, operator evidence scripts,
  JS/Python/Swift/Kotlin/Java Android SDK prover surfaces, EVM/TRON contract
  smoke, and core bridge-proof admission.
- Substrate route-canary evidence now publishes the finalized runtime code hash
  alongside the finalized head and runtime versions in public readiness JSON;
  release-bundle verification rejects zero or governed-hash-reused
  finalized-head/runtime-code canary fields before release notes can pass.
- The focused SCCP production corridor is now captured by
  `scripts/check_sccp_production_corridor.sh`, with phase selection for the
  Rust verifier crate, operator evidence scripts, web/Python/Swift/Kotlin/Java
  Android SDK proof generators, the EVM/TRON Groth16 contract smoke, and core
  bridge-proof admission target. The Java Android phase now matches the current
  test surface by running the
  main-method SCCP classes through `GradleHarnessTests` and the Solana prover
  through its direct JUnit selector, with the evidence-scripts phase also
  running the corridor runner self-check so phase drift is caught before
  release validation. The Swift phase now runs the Torii bridge-proof submit
  payload test alongside the prover/source-state batch, so iOS release
  evidence covers the final EVM/TRON user-prover submission package handed to
  Torii. The runner can now print the exact selected command plan with
  `--dry-run`, so operators can review heavyweight Rust, mobile, and
  EVM/TRON contract-smoke phases before executing the production corridor.
  Gradle-backed Kotlin and Java Android phases now also fall back from explicit
  `JAVA_HOME` to the repo-local JDK bundle, macOS `java_home`, and Homebrew
  `openjdk@21`, so local mobile SDK corridor runs do not silently execute with
  an empty Java path. The GitHub Actions attachment now uploads one
  `sccp-production-corridor-<phase>` log artifact per phase so strict release
  reports can bind CI transcripts by byte length and SHA-256 digest. The local
  runner can now produce the same
  strict per-phase transcript layout with `--log-dir
  dist/sccp-production-corridor`, so release rehearsals no longer depend on
  manually teeing each selected phase. Public release-bundle verification also
  rejects noncanonical manifest and report SHA-256 text, keeping artifact
  bindings to lowercase 64-character digests.
  `scripts/sccp_release_readiness_report.py` now converts the all-lanes
  evidence bundle plus per-phase corridor results, including the structured
  release checklist, into fail-closed Markdown or JSON release notes for
  governance review. Those reports now bind every input evidence file by byte
  length and SHA-256 digest; in strict release mode they also require a hashed
  production-corridor artifact for every passed phase, with `all=<log>`
  supported for full-run transcripts. They can also consume the same per-phase
  log directory layout produced by the local corridor runner's `--log-dir`
  option or by downloaded CI artifacts, so release notes and the self-contained
  bundle builder use the same phase-transcript source format. User-prover
  submission surfaces now carry machine-readable `sdk_helper_symbols` lists as
  well as rendered helper text, and the public release-bundle verifier rejects
  drift between those fields so web/mobile proof-generation coverage remains
  auditable. Those surfaces now also require `core-admission`, preventing
  portal/mobile proof generation from being marked validated until the
  generated proof path reaches on-chain admission. Strict reports
  now inspect each passed phase artifact for the exact corridor phase marker,
  the non-dry-run completion sentinel, and the expected command fragments inside
  the claimed phase block plus phase-specific success markers, so declared
  passed status cannot be backed by an arbitrary marker-only hashed file,
  command-only transcript, or transcript with commands under another phase
  marker. The corridor runner self-check now compares the same
  required-fragment table against full `--dry-run` phase output, keeping release
  evidence expectations synced to the actual runner command plan. Report tests
  cover blocked evidence, missing strict phase artifacts, forged phase logs,
  missing-command phase logs, wrong-block command phase logs, downloaded
  phase-artifact directories, and a complete synthetic governed bundle with
  every corridor phase marked passing and bound to a corridor log. The report also renders a
  per-lane cryptographic evidence table so public release notes expose the
  source material, source deployment, destination binding, source-gate hash and
  audit hashes, route allowlist, route canary hash, and canary evidence source
  behind each ready lane. The
  all-lanes gate also rejects cross-lane route-canary hash aliasing against
  another lane's governed source, destination binding, or route allowlist
  hashes.
  `scripts/sccp_release_bundle.py` now turns the same strict inputs into a
  self-contained public release-note attachment directory containing the
  Markdown/JSON readiness report, all-lanes summary JSON, copied evidence
  TOML, copied corridor logs, `sccp-release-notes-attachment.md`, and a
  SHA-256 manifest; the evidence-scripts corridor tests that declared-only
  phase status cannot produce a production bundle. Ready bundles now run the
  strict release-bundle verifier against their generated output before the
  builder reports success, so report/manifest/all-lanes drift fails during
  release packaging instead of only during later review. The builder now refuses
  dangerous `--force` output targets and refuses to replace a directory that
  contains the input TOML or phase transcript sources, preventing evidence loss
  during release packaging. Successful production-ready bundle generation now
  prints the verified `manifest_sha256` root directly, and reviewers can run
  `scripts/sccp_verify_release_bundle.py` against the published bundle to
  recompute every attachment hash, emit the verified `manifest_sha256` root for
  archival release review, and catch extra manifest artifacts that are not
  referenced by the readiness report, unknown corridor evidence phase keys,
  skipped or missing required corridor phases hidden behind top-level ready
  flags, non-canonical phase-log destinations, copied TOML evidence drift,
  tampered logs, symlinked manifests or artifacts, unsafe manifest paths,
  unmanifested or omitted required/phase artifacts, non-canonical
  manifest/readiness-report/summary JSON serialization, duplicate keys in
  public JSON roots, manifest artifact-order drift from the bundle builder's
  public attachment order, release notes that omit the manifest handoff, embedded
  report/summary drift, empty or non-object report/summary JSON roots,
  malformed readiness sections, missing or empty copied input-artifact
  lists, malformed or duplicate input-provenance paths, input-provenance drift
  from the copied evidence artifacts, copied evidence layout drift from
  `evidence/NN-*.toml`, non-canonical readiness-report artifact paths, missing
  or unknown manifest/readiness-report top-level fields, manifest readiness
  header drift from the report and summary, unknown embedded or standalone
  all-lanes summary root or lane fields, malformed all-lanes required-domain
  or blocker scalar lists, all-lanes required-domain drift from published lane
  domains, all-lanes domain roster or chain-label drift from the production
  remote lanes, non-ready or blocked all-lanes root or lane summaries,
  missing-record lane flags, blocked release-checklist items, malformed all-lanes lane
  record/hash/source-gate/destination-binding/route sections, zero governed
  source/destination/route hashes, zero destination bridge addresses, missing
  or misplaced lane-specific destination binding network/bridge fields,
  empty/zero/unbacked required source-adapter gate hashes, missing or zero
  required gate audit hashes, unexpected or missing lane-specific gate audit
  keys, blocked required source-adapter gates, non-required lanes carrying gate
  material, and ready source gates with blockers in public all-lanes lane
  summaries, malformed
  lane-specific route-canary transcript sections, expected destination/route
  hash drift, route-canary evidence hashes that replay governed
  source/deployment, destination, route, lane-specific canary hash roles,
  another lane's canary evidence hash, or another lane's governed hash roles,
  EVM-family route-canary zero transaction/public-input words or
  reused route-canary hash roles, including finality-height replay,
  Solana route-canary zero or non-canonical ProgramData addresses,
  TON zero or governed-hash-reused live-account route-canary hashes,
  Substrate-family route-canary zero or governed-hash-reused
  finalized-head/runtime-code hashes,
  TRON zero owner/recovered route-canary addresses, zero transcript words, zero
  route-canary binding hashes, reused canary hash roles including
  finality-height replay, or recovered signer drift from the transaction owner,
  route-canary route/destination hash drift from sibling lane evidence,
  zero cryptographic evidence row hashes, cryptographic evidence row
  domain/chain or per-field source/destination/source-gate/route/canary drift from
  embedded lane rows, unknown
  manifest
  or report artifact fields, malformed artifact byte/hash JSON types, malformed
  readiness/checklist boolean JSON types, unknown or blocked corridor root fields, unknown
  or malformed release-checklist fields, unknown or malformed portal/mobile
  submission-surface fields, report/summary drift from verifier-owned direct
  recomputation of the copied evidence TOML, Markdown readiness-report drift
  from the JSON report,
  release-checklist drift from the embedded all-lanes evidence, release-notes
  attachment drift from the verifier-owned canonical manifest/report table,
  non-canonical public JSON root serialization or duplicate JSON root keys,
  manifest artifact-order drift from the canonical release-bundle order,
  user-prover submission-surface drift from the corridor phase results, and
  missing, duplicate, unknown, malformed, unbound, or extra-field per-lane
  cryptographic evidence metadata. The verifier also requires those public
  cryptographic rows to cover every required production domain exactly once,
  keep exact domain/chain types and canonical bytes32 hash text before
  recomputing the public
  cryptographic evidence table from the embedded lane evidence and emits
  field-specific failures for any source-material, source-deployment,
  destination-binding, source-gate required flag/hash/audit hashes,
  route-allowlist, route-canary hash/source, or canary binding mismatch, so a
  release note cannot drift from the governed source, destination, source gate,
  route, or canary hashes that passed all-lanes preflight, and it
  revalidates each copied phase log's canonical path plus corridor marker,
  completion sentinel, phase-block command fragments, and phase-specific
  success markers during public bundle review.
  The report now also renders the user-prover SDK submission surfaces for each
  production lane, distinguishing EVM/TRON Torii bridge-proof submit payloads
  from native Solana instruction, TON BOC, and Substrate runtime-call envelopes
  that portal/mobile provers submit on-chain. Each surface row uses the
  user-side proof backend labels consumed by the SDK request builders
  (`sccp-solana-recursive-mainnet-v1`, `ton-contract-v1`,
  `substrate-runtime-v1`, `evm-groth16-bn254-v1`, and
  `tron-groth16-bn254-v1`) and is tied back to the required JavaScript, Python,
  Swift, Kotlin, Java Android, and core-admission corridor phases, with EVM/TRON
  additionally requiring contract-smoke coverage. The Solana destination
  manifest still binds the `solana-program-v1` target verifier backend, while
  the user-prover surface advertises the recursive backend id consumed by
  browser/mobile proof requests; release-bundle verification now rejects any
  blocked submission-surface row or non-empty validation blocker before the
  surface can be published as validated.
- The all-lanes evidence preflight now emits an explicit `release_checklist`
  that separates required lane records, governed deployment evidence, route
  allowlist binding, live route canary evidence, and unresolved blockers for
  release automation.
- Complete the first-release Offline Bearer Cash pilot over the ZK note and
  nullifier engine. Swift, Kotlin, and Java Android now expose the Bearer Cash
  v1 wallet, note, receive-request, payment-token, ACK, text-codec, and policy
  names; QR/NFC/Nearby app payloads use only the
  `wallet-offline-bearer-cash-*` prefixes; and shared fixtures publish
  `offline_bearer_cash_v1` policy defaults for custody hops, lineage steps,
  QR/stream payload limits, and Android one-use-key pool sizing. Torii no
  longer carries legacy offline transfer/revocation compatibility routes or
  MCP aliases.
  Shared chain-side `OpenVerifyEnvelope` admission now requires exact active
  verifier-key commitment binding and canonical empty auxiliary bytes for
  generic `VerifyProof`, governance voting proofs, STARK shielded
  transfer/unshield wrappers, IVM-proved overlays, IVM host registered-key
  verify syscalls, Kaigi privacy proofs, RAM-LFE proof receipts, identifier
  proof receipts, confidential-transfer-v2 transfer/unshield admission, and
  the Offline/Kagemusha flows. Private Kaigi fee admission validates its
  fee-binding auxiliary metadata at the transaction boundary and then
  canonicalizes the internal `ZkTransfer` proof to empty auxiliary bytes, while
  anonymous escrow close prechecks validate the confidential-transfer-v2 proof
  envelope before trusting parsed input commitments. The shared Halo2 IPA
  backend verifier also rejects non-empty auxiliary bytes and zero or mismatched
  envelope verifier-key hashes before proof verification, so direct verifier
  callers inherit the same fail-closed baseline; low-level backend dispatch
  also rejects proof boxes whose embedded backend label differs from the
  requested verifier backend. The lightweight preverify/dedup cache also
  decodes recognized `OpenVerifyEnvelope` wrappers and rejects malformed
  backend tags, auxiliary bytes, zero verifier-key hashes, and verifier-key
  commitment mismatches before cache insertion, while Groth16, Halo2/BN254, and
  Halo2/KZG labels remain unsupported before dedup insertion, preventing failed
  preverify attempts from poisoning later valid proofs. The checked verifier
  guardrail wrapper rejects the same trusted-setup labels before backend
  dispatch.
  The production audit path is now topup-anchored and rejects unbound input
  claims, exact-claim mutations under an issued topup certificate, hidden output
  commitments, cross-asset audits, and public amount mismatches; audit output
  certificates are signature-checked against their declared output account
  before lineage is issued. Audit inputs now require both the exact issued-claim
  replay key and an issued note-commitment replay key from the online-to-offline
  topup or a prior audited output before proof verification. `RedeemOfflineNote`
  applies the same source-commitment anchor before final
  redemption, so claim-only metadata cannot redeem a note whose commitment was
  never issued by topup or prior audit lineage. Audit output
  certificate replay keys are checked against existing topup/audit lineage before
  recursive proof verification, so a one-use certificate anchored by the
  online-to-offline topup cannot be recycled as a new bearer output. Note
  commitments are also replay-checked across both topup issue and audit-output
  domains, so commitments cannot move between online-to-offline loading and P2P
  bearer outputs.
  Recursive proof envelopes now require exact active verifier-key commitment
  binding, inline verifier-key length consistency, the literal canonical
  `offline-note-recursive` circuit id with alias spellings rejected, canonical
  empty auxiliary bytes, and shared trusted-setup/developer-only
  backend classification before verifier-registry lookup.
  Verifying-key registry admission now rejects inline verifier records with
  inconsistent published key lengths on both register and update, and rejects
  explicit trusted-setup backend labels such as Groth16, Halo2/BN254,
  Halo2/BLS12, and Halo2/KZG before they can enter registry or proof-attachment
  admission; standalone setup labels such as `kzg`, `bn254`, `bn256`, and
  `bls12_381`, plus colon-delimited profiles such as `halo2/ipa:kzg`, are now
  caught by the same shared classifier before broad allowlists can admit them.
  Generic proof attachments also reject developer-only labels before
  envelope matching, and STARK/FRI registry admission applies the same
  trusted-setup label rejection even for keyless records. Verifier-key
  register/update admission also rejects developer-only labels containing
  `debug` or `mock`, including legacy seeded records attempting to refresh
  through update. IVM host verifier snapshots and Torii's
  non-consensus proof/prover worker enforce the same trusted-setup and
  developer-only label policy before
  syscall proof verification or broad backend allowlist matching, and Torii
  prover-worker backend mismatches now stop before verifier-registry lookup.
  The core preverify cache and guardrail dispatch wrappers also reject those
  developer-only labels before dedup insertion or verifier dispatch.
  Torii-generated IVM proof
  attachments include the checked verifier-key commitment for downstream
  proof-submission binding.
  The `zk-preverify` block sidecar path records verified trace digests only;
  the background trace lane revalidates queued traces but no longer emits
  `zk-trace/mock-proof` artifacts while the real transparent IVM trace prover
  remains future work.
  `KagemushaTransfer` is now the chain-side shielded
  offline-offline instruction: it is default-on through settlement config, with
  real execution coverage asserting the default-enabled/non-legacy state; it uses
  the existing ZK asset nullifier/commitment/root accumulator, requires an
  asset-bound confidential-transfer-v2 Halo2 IPA verifier and root hint, rejects
  trusted-setup proof labels, checks the submitted `OpenVerifyEnvelope` backend
  tag, literal
  `halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified` circuit id,
  schema, and verifier-key hash against the active asset binding, rejects
  normalized confidential-transfer-v2 circuit aliases before proof decoding,
  requires inline verifier-key bytes with matching length, commitment, non-zero
  proof-size cap, active circuit/version index, and the canonical
  confidential-transfer-v2 semantic circuit key before proof envelope decoding,
  and caches canonical confidential-transfer-v2/unshield verifier and proving
  keys so production guard/prover paths do not repeatedly regenerate
  no-trusted-setup Halo2 IPA keys. Canonical confidential verifier-key
  envelopes also carry a `CID1` circuit-id TLV, keeping unshield v2/v3
  commitments separate even when the raw Halo2 payloads are structurally
  parseable across related circuits, and the Halo2 IPA verifier rejects a
  verifier-key envelope whose `CID1` names a different circuit than the proof
  envelope. It leaves legacy bearer-audit forcing available only as an explicit
  migration fallback.
  Offline recursive prover entry points, proving-key derivation, and chain-side
  verifier resolution now also pin inline verifier keys to the canonical
  `offline-note-recursive` semantic circuit key, rejecting self-consistent
  forged records or local keys before proof creation or backend verification.
  Offline recursive verifier-key envelopes also carry a `CID1` circuit-id TLV,
  and the backend verifier rejects a raw Halo2 verifier-key replay whose `CID1`
  names a different circuit family.
  Compact multi-hop Kagemusha tokens now have a deterministic folded
  public-input transcript in the Rust data model; the transcript canonicalizes
  bounded private hops, rejects duplicate nullifiers/commitments and root
  discontinuities, and binds chain id, asset definition, roots, hop count, and
  aggregate folded-hop digests. The folded statement now carries a proved
  aggregation-mode column; checked transparent pre-fold v1 is accepted, while
  reserved recursive aggregation modes are rejected until their in-circuit
  verifier exists. A Poseidon2 aggregation transcript digest is now derived from
  the same canonical hop sequence as a hash-friendly public accumulator for that
  future recursive verifier. Checked fold construction verifies each private hop
  proof and binds its verified public-input statement plus verifier-key
  id/commitment and a Poseidon2 digest of the verifier-key backend/bytes before
  hashing it into the transcript; optional envelope-hash metadata must match
  the submitted envelope bytes before private hops or chain-side transfers are
  accepted, and raw checked folding enforces the confidential-transfer-v2
  literal circuit id with alias spellings rejected alongside the proof-size cap,
  per-hop shape bounds, root continuity, duplicate-set checks,
  non-zero set entries, mandatory verifier-key commitment and verifier-key-id
  metadata, non-empty verifier-key bytes, canonical confidential-transfer-v2
  verifier-key bytes, canonical empty envelope auxiliary bytes, and the 64-hop
  compact-token corridor before parsing private-hop envelopes. Chain-side
  Kagemusha transfers now apply the same duplicate/non-zero set invariants
  before proof envelope decoding.
  The proof-statement
  preimage is exposed in the data model as `KagemushaProofPublicInputsStatement`
  and hashed through the canonical Norito/Poseidon2 helper, which now rejects
  non-empty envelope auxiliary bytes, zero verifier-key hashes, and Halo2 IPA
  confidential-transfer-v2 circuit-id aliases before transcript material is
  derived at the core verifier helper. The public data-model API enforces the
  shared auxiliary-byte and verifier-key hash rule, so SDKs and future recursive
  circuits share the same canonical target format. The public Kagemusha
  transcript helpers also reject
  unsupported, trusted-setup, and developer-only backend labels before hashing
  per-hop verifier-key material or folding verifier-key ids. The
  proof-statement helper now also rejects empty circuit ids, schema bytes,
  missing or empty instance columns, empty verifier-key bytes, and empty
  folded-hop verifier-key id names before they can become wildcard transcript
  material. The shared STARK/FRI
  classifier also rejects the profile-less `stark/fri/` prefix, whitespace-only
  STARK/FRI profile suffixes, padded or embedded-whitespace profile labels,
  punctuation-bearing or nested suffixes, non-ASCII suffixes, trusted-setup
  STARK/FRI profiles such as KZG, BN254, and BLS12, and any
  STARK/FRI profile containing developer-only `debug` or mock labels before
  verifier admission reaches proof decoding, and Torii proof/prover paths
  mirror that rule while fatal prover-worker attachment classification errors
  return before registry lookup. The shared trusted-setup classifier also
  covers standalone KZG/pairing labels and colon-profile setup labels with
  ASCII-case-insensitive matching, so registry admission, preverify, guardrails,
  and Torii's broad prover allowlists all fail closed on mixed-case forms such
  as `halo2/ipa:KZG` or `halo2/ipa:Mock-Proof`. It also tokenizes setup markers
  across every non-alphanumeric delimiter, so padded and punctuation-spliced
  setup labels such as `halo2/ipa: KZG`, `stark/fri/prod;kzg`,
  `stark/fri/prod+bn254`, and `stark/fri/prod-bls12-381` fail closed at the
  same boundaries. The classifier also normalizes delimiter-inserted setup
  spellings such as `stark/fri/prod-bn-254`,
  `stark/fri/prod-groth-16`, and `stark/fri/prod-k-z-g` before broad
  STARK/FRI profile admission. Developer-only markers are normalized the same
  way, so delimiter-inserted `d-e-b-u-g` and `m-o-c-k` spellings cannot pass
  production allowlists. Gas metering and generic proof envelope metadata helpers now
  apply the same gate before decoding pre-validation Halo2 metadata. Kotlin/JVM
  and Java Android Offline Note recursive proof models now trim verifier/proof
  backend metadata and reject malformed verifier-key separators before SDK
  proof-binding validation.
  Compact folded-token verification now also has
  explicit final-proof coverage rejecting trusted-setup and developer-only
  backend labels in both direct and record-backed verifier paths. Direct
  Poseidon2 aggregation transcript hashing now validates
  canonical mode/count/index/root continuity, sorted non-zero folded sets,
  duplicate-free membership, and
  supported transparent verifier-key backends before hashing, and rejects
  zero proof public-input digests, verifier-key commitments, or verifier-key
  Poseidon2 digests as wildcard binding material. The folded public-input model
  also rejects zero or over-64 hop counts, all-zero initial/final roots, and
  unchanged hop/public root transitions, and all-zero aggregation transcript
  digests during data-model context validation, and exposes a 1 KiB encoded-size
  budget plus `norito_encoded_len()` helpers so
  mobile transports can enforce compact QR/NFC payload corridors before adding
  backend proof bytes; folded-context validation rejects over-budget public
  transcripts. The Poseidon2 aggregation statement is exposed publicly as
  `KagemushaPoseidonAggregationTranscriptStatement` with a canonical builder
  and digest helper, plus host-side projection helpers that recompute every
  folded public-input digest column from a full aggregation statement. This
  gives SDKs and future recursive circuits the same canonicalized target layout
  and catches transcript/public-input mismatches before proof generation. The
  high-level compact-token
  prover uses that checked path before
  emitting the first
  `kagemusha-folded-v1` transparent Halo2 IPA proof, which proves and verifies
  the 30-column folded public statement without a trusted setup, constrains the
  public-input hash, initial/final roots, and aggregate digest columns to be
  non-zero inside Halo2 via inverse witnesses, proves the final folded root
  differs from the initial root with a selected-limb inverse witness, and pins
  the final folded proof envelope to canonical empty auxiliary bytes and the
  literal `kagemusha-folded-v1` circuit id. Folded-token proving, proving-key
  derivation, and direct or record-backed token verification now also require
  the canonical `kagemusha-folded-v1` semantic verifier key before backend
  verification, so a self-consistent forged verifier record cannot substitute a
  recursive aggregation or other cross-circuit key. Folded verifier-key
  envelopes carry a `CID1` circuit-id TLV too, and backend verification rejects
  a structurally compatible raw Halo2 key replayed under another circuit id. The
  Pasta circuit module now also contains reusable non-native foundations for the
  future in-circuit Vesta/Fq IPA verifier: a `u64` limb decomposition gadget
  that proves 64 boolean
  little-endian bits and rejects high residue above bit 63, a native Pasta/Fp
  scalar decomposition gadget that supports public or private scalar exposure,
  binds the scalar to four private `u64` limbs, proves the canonical 255-bit
  representation below the Fp modulus, and rejects `value + modulus` aliases, a
  canonical Vesta/Fq range gadget that proves four limbs are below the Vesta
  base-field modulus through a private slack and borrow chain, modular Vesta/Fq
  addition with
  unreduced-sum and reduction carry-chain checks, and modular Vesta/Fq
  multiplication with schoolbook product limbs, private `u128` carry chains, and
  a private canonical quotient, plus a Vesta affine on-curve check that links
  public `x/y` coordinates to private `x*x`, `y*y`, `x^2*x`, and `x^3 + 5`
  witnesses and a distinct affine point-addition gadget that composes on-curve
  checks with private denominator-inverse, slope, and output-coordinate
  equations, plus an affine point-doubling gadget that proves an invertible
  `2*y(P)` denominator and links `lambda * (2*y(P)) = 3*x(P)^2`, and a
  point-or-identity validity gadget with canonical `(0, 0, 1)` identity
  encoding. Complete point-or-identity addition now covers identity
  passthrough, inverse-pair output identity, doubling, and distinct affine
  addition under one-hot branch selectors. A conditional-add layer now binds a
  private selected addend to a boolean scalar bit, and the first bounded
  scalar-multiplication wrapper links public scalar-limb decomposition, the
  addend doubling ladder, private accumulator steps, and public base/output
  point encodings. A native-scalar scalar-multiplication wrapper now consumes
  canonical Pasta/Fp scalar decomposition bits directly, enforces high-bit
  zeroing for bounded widths, and proves the same private addend-doubling ladder
  from the public base. A fixed-window Pasta/Fp scalar decomposition gadget now
  proves deterministic little-endian window digits for the production
  windowed-MSM path, links every digit bit to the canonical private scalar bit,
  and constrains high scalar bits above the configured window width to zero. A
  non-native Vesta fixed-window point selector now proves that a selected
  private point-or-identity comes from a private `2^WINDOW_BITS` table through a
  quadratic binary selection network. A companion table-derivation gadget now
  proves the private table is exactly `[0, B, 2B, ...]` for a public base point
  by linking entry zero to identity, entry one to the public base, and later
  entries to a complete-add chain. A fixed-window native-scalar multiplication
  wrapper now composes scalar windows, shifted-base tables, selectors,
  per-window base doublings, and selected-point accumulation into a public
  `output = scalar * base` statement. The remaining windowed-MSM layer composes
  multiple windowed scalar-multiplication terms into one public multi-scalar
  accumulator. A bounded native-scalar Vesta MSM wrapper now composes
  private canonical Pasta/Fp scalar witnesses, public base encodings, per-term
  private scalar-multiplication ladders, and a private running sum into one
  public output point, rejecting public base/output substitution, scalar-bit
  tampering, noncanonical scalar aliases, broken double ladders, and unchained
  private MSM accumulators. The first IPA-specific composition wrapper now
  proves the final verifier comparison `Q = a*G + b*H + (a*b)*U` by reusing the
  three-term bounded MSM and constraining the third scalar to the native-field
  product of the first two, so a self-consistent MSM cannot forge the IPA
  product term. The per-round accumulator update `Q' = x^2*L + Q + x^{-2}*R`
  now uses the fixed-window MSM path with private canonical `x` and `x^{-1}`
  witnesses constrained as inverses and linked to the MSM scalars `x^2`, `1`,
  and `x^{-2}`. Generator folding now also has a shared-challenge wrapper
  proving `G' = x^{-1}*G_L + x*G_R` and `H' = x*H_L + x^{-1}*H_R` with two
  linked fixed-window two-term MSMs. The native-field IPA `b`-vector fold
  `b' = b_L*x^{-1} + b_R*x` now has public-input scalar and fixed-size
  segment-vector gadgets with one shared private canonical challenge pair and
  adversarial coverage for inverse, input/output, and noncanonical scalar
  tampering. A multi-round `b`-vector reduction gadget now folds the whole
  power-of-two public vector to the final public scalar while keeping
  intermediate vectors private and canonical; its round challenges and inverses
  are public circuit inputs linked to private canonical decompositions so the
  recursive circuit can bind externally projected Fiat-Shamir challenges. The
  native transparent IPA verifier now derives the same projection and rejects
  substituted `proof.b_final` values. Native IPA vector commitments now use backend-level
  deterministic MSM hooks, with Pallas and BN254 using `halo2curves::msm_best`
  and simple backends retaining the generic deterministic fallback. A
  fixed-window native-scalar MSM wrapper now composes private canonical scalar
  windows, shifted-base tables, table selections, private per-term outputs, and
  the final public multi-scalar accumulator, with adversarial coverage for
  substitution and splice attacks. The IPA final comparison now also has a
  fixed-window `Q = a*G + b*H + (a*b)*U` wrapper with the same third-scalar
  product-link invariant. The bounded, fixed-window, and shared-table
  fixed-window final MSM wrappers now also have explicit identity-output
  coverage, proving that the verifier comparison can end at the canonical point
  at infinity without leaving the complete-add/point-or-identity path. The
  composed one-round/generic verifier wrappers now feed the round accumulator,
  generator folds, and final comparison through the fixed-window MSM path. The
  native accumulation projection also rejects
  mismatched challenge inverse witnesses.
  A one-round in-circuit verifier composition slice now shares one canonical
  challenge/inverse pair across `b` folding, the `Q` accumulator update,
  generator folding, and final MSM comparison, with direct advice links for
  folded `b`, `Q'`, `G'`, and `H'`. A native transparent IPA
  round-transcript projection helper records the `ipa.n` state boundary, each
  round's `L/R` bytes, round-byte digest, transcript states, challenges,
  challenge inverses, and final transcript state, and a native verifier
  accumulation projection records `Q`, folded `g/h`, challenge squares, final
  folded generators, and the final expected term. A combined native verifier
  witness now validates those transcript, reduction, accumulation, and final
  scalar projections together for future recursive-verifier witnesses, all
  without adding a trusted setup. A field-friendly transcript-binding projection
  now maps the SHA3-validated transcript header, complete round projections,
  challenge/inverse pairs, and final transcript state into Pasta/Fp scalars and
  folds them through a transparent Pow5 accumulator; a matching native Pasta/Fp
  circuit enforces that accumulator over public projection/challenge inputs and
  rejects public substitution or intermediate-state tampering. The generic
  multi-round non-native Vesta IPA verifier now composes that accumulator and
  links its challenge rows back to the verifier's decomposed `b`-reduction
  challenge columns, so self-consistent transcript witnesses cannot be spliced
  onto a verifier using different challenges. The host bridge now accepts native
  Pallas IPA verifier witnesses, validates their transcript projection,
  re-derived transcript binding, `b`-reduction and accumulator projections,
  round ordering, and canonical compressed point encodings through a cheap
  preflight path, recomputes the native Pallas `b`, `Q`, `G`, `H`, and
  final-term fold relations with the deterministic optimized Pallas MSM backend,
  and translates their scalars and compressed Vesta points through canonical
  byte encodings before building the recursive Vesta verifier witness. The same
  bridge now validates ordered batches of native Pallas
  verifier witnesses and emits a compact streaming Poseidon2
  domain-separated aggregate digest that binds the transparent parameter
  fingerprint, verifier opening length, witness order, transcript projections,
  `b` reductions, accumulator folds, final terms, and proof-final scalars after
  each witness passes preflight. The data model now exposes a
  reserved-mode recursive aggregation evidence statement that
  Norito/Poseidon-binds that batch digest, parameter fingerprint, and canonical
  `pallas-ipa-transparent-v1/vesta-recursive-fixed-window-85x3` verifier-witness
  profile plus the declared verifier opening length to the same ordered hop
  transcript while keeping mode `2` rejected for compact-token admission, with
  a Poseidon2 aggregation transcript digest that accepts checked mode `1` and
  reserved mode `2` but rejects unknown modes, plus Norito roundtrip,
  decoded-profile/opening-length/schedule/base, and truncated-archive negative
  coverage plus unsupported or non-power-of-two opening length, zero table
  commitments, empty-transcript, over-cap, duplicate-nullifier, and
  duplicate-commitment rejection. Core record-backed
  evidence builders now enforce active WSV-style confidential-transfer-v2
  verifier records, verify every private hop proof, reject mismatched witness
  counts, unsupported opening lengths, all-zero native batch metadata, or
  opening-length/schedule-digest mismatches before hop proof decoding, and then
  bind the batch preflight digest to the canonical hop transcript for both
  borrowed and serializable record bundles. A public Pallas
  IPA batch preflight helper now accepts only the current production
  no-trusted-setup width corridor `2..=128` plus the 64-hop compact-token cap,
  keeps the aggregate batch digest on the same Poseidon2-backed transcript
  family as reserved recursive evidence, and the combined record-backed
  builders can take native verifier witnesses directly, re-derive the stored
  batch digest with the ordered checked-hop proof hashes, and reject detached,
  wrong-width, or spliced batch evidence before hop proof decoding. The native
  batch digest now also binds the recursive fixed-window table profile and the
  Poseidon2 digest of the deterministic shared-table schedule plus a Poseidon2
  digest of the ordered fixed-window table bases used by the validated native
  witnesses, and a proof-derived Pallas opening-envelope path reconstructs
  native verifier witnesses from the transparent IPA proof envelope before
  applying the same preflight and hop-proof-hash binding. That path now applies
  the Kagemusha `2..=128` power-of-two opening corridor and `k = 7`
  transparent-envelope resource limit before verifier-witness derivation, so
  unsupported or oversized opening envelopes, mismatched wire versions,
  generator vectors, and proof-round vectors reject before parameter/proof
  reconstruction. Hop-proof hash count mismatches now reject before native
  witness preflight or proof-derived envelope witness derivation, keeping
  detached reserved-evidence batches from forcing unnecessary verifier work.
  Kagemusha proof-derived preflight also rejects empty or over-128 byte
  transcript labels and requires non-zero verifier-key commitment,
  public-input schema, and hop-domain metadata before verifier-witness
  derivation, so detached generic polynomial-opening envelopes cannot enter
  reserved recursive evidence.
  The record-backed
  proof-derived path
  now also requires each Pallas opening envelope to carry hop-derived transcript
  metadata: verifier-key commitment, confidential-transfer-v2 schema hash, and a
  Poseidon2 domain tag over chain, asset, hop index, roots, nullifiers, outputs,
  proof hash, public-input digest, and verifier-key binding. This closes
  profile-string/table-accounting substitution, detached native-opening witness
  replay, and unbound opening-envelope replay at the reserved-evidence layer.
  Deriving each IPA verifier witness inside the recursive circuit from the
  corresponding compact-hop Halo2 proof envelope remains part of the mode-2
  recursive circuit work. A cheap
  production-layout guard now pins
  the `n = 128` recursive verifier shape (seven rounds, `[64, 32, 16, 8, 4, 2,
  1]` generator-fold layers, 85-by-3 scalar coverage, and 262 represented
  windowed MSM gadgets), and a fixed-window table plan pins the shared-table
  target at 532 table families versus 90,440 naive point-table copies
  (723,520 duplicated point rows) with `trusted_setup_required = false`. Both
  guards reject unsupported widths; full production-width witness
  materialization remains too large for regular tests until the circuit uses
  shared/compressed fixed-window table evidence. A deterministic shared-table
  schedule and Poseidon2 schedule digest now enumerate those 532 table families
  and 45,220 shifted-window tables explicitly, giving the compressed recursive
  verifier work a no-trusted-setup commitment target instead of only a row-count
  estimate. A concrete shared-table manifest now assigns those families to
  contiguous shared-row ranges, binds that row layout with Poseidon2, and keeps
  `trusted_setup_required = false`; reserved recursive evidence now carries the
  opening length plus schedule, shared-table manifest, and table-base digests
  explicitly with unsupported-opening, non-power-of-two-opening,
  zero-schedule/manifest/base, and schedule/manifest-mismatch rejection before
  hop proof decoding. Native batch preflight also exposes that manifest digest,
  and hop-bound batch preflight binds it with the schedule digest and ordered
  checked-hop proof hashes. The data model now also has a proof-carrying
  recursive aggregation public-input bundle whose 43 public instance columns
  bind transparent no-trusted-setup proof metadata to the recursive evidence
  digest, aggregation transcript digest, verifier-parameter fingerprint,
  fixed-window schedule digest, shared-table manifest digest, table-base digest,
  native witness-batch digest, recursive spend proof-chain digest, reserved
  recursive verifier scalar-projection digest, opening length, witness count,
  and hop count while rejecting backend, circuit-id, public-input-hash, empty
  proof payloads, and evidence-field substitution. The proof-bundle guard is
  pinned to the
  canonical transparent Halo2 IPA/Pasta recursive aggregation circuit; supported
  STARK/FRI labels remain hop-transcript material only and cannot stand in for
  this in-tree recursive proof. Core now also pins the schedule and shared-table manifest
  digests to the declared opening width during recursive proof generation,
  preverification, and the transparent semantic circuit, so a self-consistent
  forged envelope cannot swap those verifier-layout commitments. Recursive
  aggregation preverification also rejects cross-circuit verifier keys even when
  the supplied inline key, verifier-key commitment, and proof-envelope `vk_hash`
  are internally consistent. Recursive verifier-key envelopes carry `CID1`
  circuit-id TLVs, and backend verification rejects structurally matching raw
  verifier-key payloads whose `CID1` names another circuit family. Core now
  prevalidates that bundle against active Kagemusha verifier records by checking
  the non-empty transparent Halo2 IPA envelope, canonical circuit id, verifier-key hash,
  public-input schema,
  empty auxiliary bytes, Pasta instance columns, proof-size cap, inline key
  length, and verifier-key commitment, and it exposes a canonical verifier-record
  helper for supplied transparent Halo2 IPA recursive aggregation key bytes.
  The detached-evidence prover and raw metadata evidence builder are
  crate-private implementation helpers, leaving the public proof-bundle API on
  the record-backed native Pallas preflight/open-envelope paths. The
  ZK1 public-instance parser remains bounded but now admits this 43-column
  recursive aggregation envelope through core and the native bridge. This
  remains a reserved evidence surface until the recursive verifier proof
  itself is complete. Recursive spendable offline cash now uses a separate
  production accumulator and Norito bundle, `KagemushaRecursiveSpendBundleV1`.
  It carries the public accumulator state, current spendable note descriptor,
  chain/asset/final-root/final-commitment binding, verifier references, hop
  count, and one recursive proof instead of prior hop bundles. Recursive spend
  redemption now binds a compact
  chain-visible top-up anchor set into the accumulator: the first-hop input
  nullifiers from the online-to-offline top-up lineage are sorted, included in
  the accumulator digest and recursive proof public inputs, and consumed
  alongside the final spendable note nullifier at redemption. This prevents two
  hidden recursive branches from redeeming from the same top-up anchor while
  preserving hop-count-independent payload size. Append hops are restricted to
  consuming the previous spendable note nullifier only, so they cannot merge
  fresh external inputs whose nullifiers are not part of the original top-up
  anchor set or create a current note that reuses the consumed append nullifier.
  Accumulator context validation also rejects forged top-up-anchor/current-note
  commitment collisions and current note spend nullifiers that collide with any
  output commitment in the hop that created the note.
  Recursive spend nullifier, output, and fold-transcript streams now use
  accumulator-only domain tags instead of the checked folded-token
  list/transcript digest tags; the C bridge and Python native redeem helpers
  also reject zero or mismatched public amounts before instruction emission. The
  first shared-table
  circuit primitive is now in
  place as well: fixed-window selection can reference an already-derived Vesta
  table directly instead of assigning a duplicate private selection-table copy,
  and shared-table native-scalar multiplication now composes that selector with
  scalar decomposition, shifted-base table derivation, window-base doubling, and
  selected-point accumulation. Shared-table multi-term MSM composition now
  chains those scalar-multiplication terms into one public MSM output while
  keeping term outputs private, and shared-table final IPA MSM composition adds
  the native-field `a * b` product link for the `U` term. Shared-table
  round-accumulator and generator-fold composition now binds those MSM scalars
  back to the transcript challenge and inverse for per-round IPA verification,
  and the one-round shared-table verifier slice now preserves the `b`-reduction,
  final-output, and folded-generator cross-links across those shared-table
  components. The multi-round shared-table verifier builder now mirrors the full
  transcript-binding, `b`-reduction, accumulator-chain, folded-generator-chain,
  and final-MSM host-link structure without duplicated selection-table copies;
  full LEN=4 MockProver coverage is present for an honest synthetic statement,
  real Pallas opening translation, public-instance substitution, `Q` splice,
  generator-fold splice, challenge splice, and final-MSM splice, but those tests
  are ignored by default because the composed non-native layout is too expensive
  for routine validation. The normal suite keeps builder-level, native Pallas
  preflight, batch-preflight, and host-link adversarial coverage active.
  The first recursive-aggregation verifier-slice composition is also in place:
  a one-hop circuit composes the recursive aggregation semantic metadata proof
  with the shared-table `LEN = 2` IPA verifier, including transcript-binding
  accumulator checks, and links the public opening length, witness count, and
  hop count to the single-hop profile. Its active coverage checks builder
  acceptance, profile and metadata-witness mismatch rejection, stale semantic
  non-zero inverse rejection, zero public digest-group rejection, native Pallas
  preflight metadata binding, preflight digest/fingerprint substitution
  rejection, zeroed preflight digest rejection, rejection when a valid Pallas
  preflight is paired with an invalid verifier witness, and rejection when a
  production Pallas preflight is paired with a non-production fixed-window
  profile. The shared-table host-link guard also enforces the expected IPA round
  count and per-round `b`-reduction and generator-fold layer widths, so a
  composed recursive verifier witness cannot omit an initial `b` layer or
  generator-fold layer and rely only on the final MSM relation. The native and
  shared-table verifier `synthesize` paths now repeat those witness/config
  shape checks before assignment, so malformed direct circuit witnesses fail
  closed instead of silently skipping omitted `b`-reduction or generator-fold
  regions. The composed circuit now also exposes a public verifier
  transcript-binding digest instance and links it to the embedded verifier's
  transcript-binding accumulator. It also exposes a
  public scalar-projection digest over that digest, the public `b`-reduction
  input scalars, challenge, inverse, and final folded `b` scalar, and
  constrains that projection with the same field-friendly Pow5 compressor.
  One-hop constructors now recompute the scalar projection from the host
  verifier witness and reject semantic public-input limb splices before circuit
  assignment.
  The shared-table verifier host-link guard also mirrors the final IPA MSM
  product relation, rejecting `a_final * b_final` product splices before a
  composed one-hop recursive verifier witness can be accepted.
  The production one-hop host API can now re-derive the Pallas preflight from
  the supplied witness and require an exact digest/fingerprint match before
  materializing the recursive Vesta verifier, so metadata from one valid
  witness cannot be paired with another self-consistent verifier witness. It
  can also re-derive the reserved hop-proof-hash-bound preflight from the
  supplied witness plus the expected hop proof hash, so one-hop mode-2 evidence
  cannot accidentally validate against the detached native batch digest. The
  hop-bound path now has a dedicated constructor separate from the detached
  native-batch constructor, so reserved evidence callers cannot materialize a
  one-hop verifier slice through the wrong preflight shape. The hop-bound guard
  now derives the native preflight through the verifier slice's declared opening
  length before binding the hop proof hash, so a LEN=4 witness cannot validate
  through a LEN=2 one-hop slice. The constructor also revalidates recursive
  semantic non-zero inverse witnesses and rejects all-zero preflight
  fingerprint, table-schedule, shared-table-manifest, table-base, or
  verifier-batch digests before accepting a composed witness.
  Digest-splice, scalar-projection side-instance splice, scalar-projection
  semantic public-input splice, and direct verifier synthesis-shape MockProver
  cases for omitted `b`-reduction and
  generator-fold layers, production fixed-window Pallas verifier
  materialization, and composed MockProver acceptance/public-count-splice tests
  remain ignored heavyweight coverage.
  This is still not mode-2 admission because the private-hop verifier batch and
  complete Poseidon2 witness-batch digest relation are not yet proved inside the
  compact-token circuit.
  The same shared-table verifier can now be constructed from proof-derived
  native Pallas verifier witnesses after the existing native Pallas preflight
  validates transcript, `b`-reduction, accumulator, and generator-fold
  consistency, and the public Kagemusha Pallas batch-preflight path now
  dispatches through the shared-table verifier entry point while exposing the
  same ordered schedule, shared-table manifest, table-base, and aggregate digest
  metadata used by reserved recursive aggregation evidence. Recursive preflight
  Poseidon2 transcripts now field-label length encodings, closing same-length
  cross-field replay before mode-2 evidence becomes admissible. Production-width
  coverage now binds the shared-table verifier shape to the `n = 128` manifest
  without materializing every recursive witness object in routine tests.
  Alias
  spellings are rejected at compact-token proving
  and verification boundaries. Derived Halo2 IPA proving keys for IVM,
  Offline Note, and Kagemusha now use Norito archives
  that bind the canonical circuit family and verifier-key commitment before raw
  Halo2 key bytes are decoded, rejecting raw or cross-circuit key material while
  preserving production key caching. Mobile and bridge callers must use the
  record-backed compact-token prover
  `connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`
  so private hops are tied to active WSV-style confidential-transfer-v2 verifier
  metadata, including canonical `offline_kagemusha` namespace, backend tag,
  circuit id, schema hash, verifier-key commitment, key length, proof-size cap,
  optional inline-key consistency, and exact record-set matching with no
  unrelated records at the FFI boundary, while raw folded-input proof
  construction stays crate-local. The final folded-token record verifier applies
  the same canonical namespace and registry metadata gate before backend proof
  verification. The older unanchored C
  symbol and Rust compact-token proving entry points remain present for ABI
  compatibility but reject even valid `KagemushaVerifiedFoldBundle` input
  without returning a token.
  Bridge ABI 6 adds recursive spend `init`, `append`, `verify`, and `redeem`
  entry points over raw Norito archives, and the C header plus Swift,
  Kotlin/JVM, Java Android/JNI, JavaScript/Node NAPI, Python/PyO3, and C#
  surfaces mirror them with empty-input and malformed-archive rejection. The
  SDK/native-output guard now also distinguishes missing native proof archives
  from zero-length archives across Python, Swift, JavaScript/Node, Kotlin/JVM,
  and Java Android, keeping native prover boundaries fail-closed without
  disabling recursive Kagemusha by default. The
  shared redeem-request validator now also rejects final redeem proof
  attachments that leave the current transparent `halo2/ipa` production
  corridor, carry inconsistent attachment/proof/verifier-key backends, have
  empty proof bytes, omit or publish a zero verifier-key commitment, or carry a
  mismatched envelope hash before native/bridge instruction construction. It
  also rejects recursive spend bundle proofs that are not `halo2/ipa` or carry
  empty proof bytes, keeping native/bridge redeem construction inside the same
  production corridor as ledger-side verifier-record admission. The
  SDK surfaces also expose a common preferred offline spend-mode selector:
  `recursive_spend_v1` when the ABI-6 recursive spend surface is available and
  `checked_prefold_v1` as the compatibility fallback. The
  recursive D2D payload benchmark records 1,454-byte fixture archives for hop
  counts 1, 2, 3, 5, 8, 13, 21, 34, 55, and 64 with a fixed 256-byte proof
  payload and asserts that archive length remains hop-count-independent. The
  recursive spend accumulator now validates that its aggregation transcript
  digest equals its lineage digest, keeping the recursive proof public input
  attached to the spend-lineage accumulator rather than a detached digest.
  Python, Swift, Kotlin/JVM, and Java Android now expose record-backed compact-token
  prover wrappers over that ABI, so mobile wallets can pass
  `KagemushaVerifiedFoldRecordBundle` Norito bytes through the native bridge
  instead of constructing preverified folded public inputs themselves. The same
  SDK surfaces now expose the ABI-6 recursive aggregation proof-bundle prover,
  which accepts record-backed bundle bytes plus proof-derived Pallas
  open-envelope archive bytes and returns an admission-neutral
  `KagemushaRecursiveAggregationProofBundle` for future mode-2 work.
  Swift, Kotlin/JVM, and Java Android Offline Note proof binding now also
  rejects substituted recursive verifier ids or proof backend labels before
  accepting wallet-side validation, keeping mobile checks aligned with the
  chain's `halo2/ipa:offline-note-recursive` trust anchor. Draft wallet and
  redeem-planner bundles now carry the explicit unsupported
  `offline-note/draft-placeholder` backend until a real proof provider replaces
  them.
  Offline note key certificates now fail closed when an exposed hardware
  usage-count limit is anything other than exactly `1`, so a certificate cannot
  claim one-use semantics while carrying a multi-use or zero-use platform
  counter. The Torii offline issuer enforces that rule while minting the
  online-to-offline topup certificate, and Swift, Kotlin/JVM, and Java Android
  constructors mirror it before wallet-side serialization. Their Torii
  issuer-response parsers also reject malformed, overflowed, or non-numeric
  certificate versions and counter values before narrowing JSON into SDK
  certificate models. Torii topup issuance also derives the wallet JSON
  certificate and `IssueOfflineNote` chain certificate from the same signed
  object, so the wallet's offline trust anchor is the exact certificate payload
  recorded by the online-to-offline transaction. Legacy Offline audit metadata now mirrors the Kagemusha
  nullifier/commitment separation rule by rejecting any byte-identical overlap
  between consumed input nullifiers and new output commitments before recursive
  proof decoding.
  Chain-side Kagemusha transfer admission also rejects any byte-identical
  overlap between consumed input nullifiers and newly created output commitments
  before proof decoding, keeping ledger admission aligned with the proof
  system's nullifier/commitment domain separation. The shared folded-public-input
  and Poseidon aggregation-transcript validators mirror that rule for same-hop
  and cross-hop overlap before compact-token or reserved recursive evidence is
  built.
  Current release evidence covers physical iOS App Attest/HCE/CardSession
  availability and Android StrongBox/KeyMint one-use-key validation. The open
  physical gap is the end-to-end cross-platform NFC/HCE payment
  exchange with both devices unlocked and ready; recursive aggregation of the
  private per-hop proofs into the compact folded-token mode-2 proof remains
  follow-up work for a later version, while spend-again-offline cash uses the
  recursive spend bundle and ABI-6 append path now. Native Pasta/Fp scalar
  decomposition, fixed-window scalar decomposition, fixed-window Vesta point
  selection, table derivation, and scalar-multiplication composition, and
  fixed-window multi-term MSM plus bounded native-scalar MSM, fixed-window IPA
  final-comparison MSM, IPA scalar/vector-fold, full `b`-vector reduction,
  generator-fold, round-accumulator, final-comparison composition, and one-round
  and generic multi-round verifier composition with transcript binding plus
  native Pallas verifier-witness translation, batch preflight binding,
  reserved-mode recursive aggregation evidence/proof-public-input binding, and a
  transparent Halo2 IPA semantic proof for the recursive evidence layout are
  present. Record-backed combined proof-bundle builders now derive evidence from
  active hop verifier records and Pallas witness/open-envelope material before
  proving it, but private-hop Pallas IPA witness verification inside the
  compact-token recursive circuit and mode-2 compact-token admission remain
  reserved, explicitly rejected wire values until that verifier evidence exists.
- Continue dependency, documentation, and release hygiene work required by LF
  Decentralized Trust project expectations.

**Next checkpoints:** governed deployment evidence, live canary evidence, and
attaching the generated SCCP release-readiness report to public release notes.

## SORA Nexus and Taira

**Status:** active pre-release hardening.

- Use the public Taira testnet to harden consensus, routing, lane-aware
  execution, data availability, operator workflows, and SDK integration.
- Complete the remaining independent-lane consensus, DA/RBC, and cross-lane
  relay validation needed for the first public Nexus release.
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
  journal once per Kura replay range while logging per-phase timings. The
  remaining checkpoint for this corridor is keeping the broadened strict soak
  in regression rotation while release packaging and operator runbooks
  converge.
- Continue native AMX hardening beyond the implemented attestation data model,
  control-plane message handling, deterministic per-leg vote cache,
  proposer-side prepare/commit gating, 4-peer convergence proof,
  queue-journal restart replay, and routing-plan projection with longer-running
  soak, fault injection, and independent participant-lane finality work.
- Keep SCCP bridge submission permissionless while requiring outbound message
  records to originate from verified IVM-proved overlays, route allowlists to
  be deployment-governed, and production activation to wait for all advertised
  lanes to have cryptographic source-chain proof adapters plus immutable
  destination verifiers. ETH/BSC production targets `evm-groth16-bn254-v1`,
  TRON/TVM production targets `tron-groth16-bn254-v1`, and the secp256k1
  attestation verifier remains direct-fixture-only; web/mobile SDK proof
  request builders now reject any EVM-family or TRON backend string outside
  those canonical Groth16 backends before invoking the app-linked prover, and
  the EVM wrapper constructor now rejects any non-Groth16 backend, proof
  families other than `stark-fri-v1`, missing or mismatched verifier-key
  hashes, non-SORA sources, and non-ETH/BSC targets before any deployment can
  advertise mismatched binding metadata. Rust destination-binding helpers now
  also refuse to derive deployable EVM bindings for the reference secp256k1
  backend. Rust manifest readiness ignores a flipped `production_ready` flag
  when the manifest backend, verifier target, or proof family has been mutated
  away from the canonical production lane, and Torii proof-material routing
  plus capability discovery now use that effective readiness check. Rust
  destination rollout readiness now also rejects padded EVM addresses, Solana
  program ids, TON raw addresses, TRON Base58Check addresses, and Substrate
  runtime entrypoints instead of trimming them into production verifier
  identities. The EVM, Solana, TON, and Substrate destination evidence
  renderers now mirror that exact-input posture for verifier identities,
  fixed-width hashes, lane selectors, and deployment metadata before they can
  emit production TOML, and the all-lanes preflight now requires the
  helper-emitted TON live account comments to remain attached to governed
  account status, account-state, and last-transaction fields. Rust,
  JavaScript, Python, Swift, Kotlin, and Java
  Android portal/mobile helpers now derive EVM-family and TRON destination
  binding hashes from governed deployment tuples; web/Python request builders
  reject mismatched raw `destinationBindingHash` values before app-linked
  prover callbacks run, Rust request/result wrappers bind the same deployment
  object into request hashes, public signal words, and envelope hashes, and
  Swift, Kotlin, and Java Android request/submission constructors can now accept
  the same derived binding object directly, reject mismatched binding metadata,
  and thread the derived hash into request hashing or verifier-call packaging.
  Swift/Kotlin/Java Android bridge-proof submit DTOs and JavaScript/Python
  Torii submit payload helpers can also be built directly from the generated
  EVM-family/TRON submission plus that governed binding object, deriving the
  on-chain `proof_bytes_hex` and destination tuple instead of requiring UI code
  to manually copy proof material. Raw EVM-family/TRON submit preflights in
  those SDKs now also recompute destination binding hashes from the supplied
  deployment tuple before any Torii request is posted, and JavaScript/Python raw
  message submissions now require message-bundle commitment metadata before
  `proofBytesHex`/`proof_bytes_hex` can be bound and posted. The JavaScript web
  package root and `./sccp` subpath now publish the same EVM-family/TRON
  bridge-proof submit payload
  builders in `dist` plus TypeScript declarations, and the Python package root
  exports the matching helpers, so browser portals and mobile-backed user
  provers can hand generated Groth16 proof submissions to Torii without
  manually copying destination deployment fields. Those dynamic helpers plus
  the Swift/Kotlin/Java Android typed submit DTO builders now require the SCCP
  message bundle commitment root, locally re-check the BN254 Groth16 tuple, and
  bind the tuple to `message_bundle.commitment.message_id`,
  `message_bundle.commitment_root`, and the SORA source-domain word before
  returning a submit payload. Torii
  production bridge-proof submit, artifact, proof-job, and runtime SCALE
  envelope generation now also require SORA-origin message bundles to pass the
  BLS-backed Nexus finality verifier before packaging, and burn bridge-proof
  submission rejects structurally valid but unsigned Nexus finality proofs.
  SORA-origin message lookup now resolves locally published/cacheable bundles
  before the non-SORA proof registry, keeping runtime proof export available for
  freshly published local messages without weakening external-source proof
  admission. Core SCCP finality admission now also runs the embedded Nexus
  finality through the BLS aggregate verifier before local block/QC anchoring,
  so structural-only finality is rejected directly on-chain.
  Dynamic JavaScript and Python EVM-family/TRON destination binding helpers now
  also reject duplicate aliases for network ids, verifier addresses,
  verifier-code/key hashes, backend/proof-family selectors, binding hashes, and
  proof-context destination-binding fields before request hashing or app-linked
	  prover invocation. Their EVM/TRON/Substrate-family proof request builders now
	  apply the same top-level guard to `publicInputs`, `bundleBytes`,
	  `sourceProofBytes`, `sourceDomain`, and `proofContext`, and Substrate proof
	  contexts reject duplicate nested binding-hash aliases. The dynamic
	  JavaScript and Python EVM/TRON proof-result wrappers and contract-call
	  submission builders now also reject duplicate aliases for request hashes,
	  envelope hashes, proof contexts, proof bytes, bundle/source-proof bytes,
	  public inputs, source domains, and public signal words before a web portal
	  or mobile backend packages user-generated proofs into counterparty calldata.
	  The JavaScript Groth16 result wrappers also copy and freeze request-derived
	  public inputs, public signal words, and proof contexts, so mutating a
	  manually supplied request object after wrapping cannot change what the
	  portal submits on-chain.
	  Their Substrate-family proof-result wrappers and runtime-call submission
	  builders now apply the same guard before packaging user-generated proofs into
	  SCALE calls for Substrate destinations. The shared dynamic JavaScript and
	  Python transparent public-input normalizers now also reject duplicate aliases
	  inside message ids, payload hashes, target domains, commitment roots,
	  finality heights, and finality block hashes before any request hash, public
	  signal word list, or submission envelope is derived. JavaScript and Python
	  ETH/EVM receipt-proof helpers now reject duplicate aliases for source
	  domains, source event digests, beacon slots, execution block/finality
	  numbers and hashes, receipt roots, beacon finalized roots, sync-committee
	  roots, receipt proof nodes, and inclusion branches, and they reject non-ETH
	  source domains before deriving ETH receipt-proof hashes. The ETH
	  sync-committee payload, transition-message, and transition-signature helpers
	  apply the same guard to committee public keys/weights/PoPs, transition
	  periods and slots, finalized roots, parent/next committee hashes, payload
	  hashes, branch hashes, transition-message hashes, signers bitmaps, aggregate
	  signatures, and nested proof weight fields before hashing. ETH beacon
	  block-header root helpers now reject duplicate slot, proposer, parent-root,
	  state-root, and body-root aliases before SSZ root derivation. JavaScript and Python
	  BSC Parlia receipt-proof, validator-set payload, validator-set
	  metadata/transition, commit-message, and commit-seal helpers now apply the
	  same guard to source domains, source event digests, validator epochs,
	  block/finality numbers and hashes, receipt roots, proof nodes, inclusion
	  branches, validator addresses/powers, validator-set storage roots, slots,
	  values, value hashes, payload hashes, metadata proof hashes, total/signed
	  power, commit-message hashes, validator keys, signers bitmaps, and
	  validator-set hash echoes before deriving BSC source proof hashes.
	  JavaScript and Python Solana message-proof, transaction-status leaf, and
	  transaction-status root helpers now reject duplicate aliases for source
	  event digests, transaction-status/receipt-message roots, transaction
	  signatures, emitter program ids, and inclusion branches before deriving
	  Solana source proof hashes.
	  Their semantic vote-account and stake-account data canonicalizers now
	  reject duplicate aliases for node/voter/withdrawer keys, collector and
	  commission fields, Tower vote slots, delegated stake,
	  activation/deactivation epochs, warmup/cooldown bytes, credit counters,
	  and stake flags before deriving AccountsLtHash account-data inputs.
	  Their epoch-stake, stake-activation, stake-account-state, StakeHistory,
	  and StakeHistory-sysvar transcript helpers apply the same guard to
	  epoch/slot fields, validator account address/hash vectors,
	  delegated-stake vectors, and StakeHistory vectors before deriving Solana
	  finality/source-state hashes.
	  Their Solana active-stake, stake-activation, and stake-history helpers also reject
	  duplicate aliases for validator public-key rosters, validator stake
	  weights, activation epochs, and deactivation epochs before deriving Solana
	  finality/source-state transcripts.
	  Their Solana account-opening, AccountsLtHash opening-normalization, and
	  account-inclusion leaf helpers now apply the same alias guard to account
	  addresses, owner program ids, rent epochs, account-data hashes, finalized
	  slots, opening objects, raw account data, raw-data hashes, and nested
	  opening addresses; if both raw account data and a raw-data hash are
	  supplied, JavaScript and Python recompute and require equality before
	  deriving the account-inclusion transcript.
	  Their opened-AccountsLtHash contribution, opened-account inclusion witness,
	  and Agave bank-hash helpers now reject duplicate aliases for opened
	  vote/stake arrays, StakeHistory sysvar fields, account-inclusion roots,
	  AccountsLtHash checksum/root fields, full AccountsLtHash bytes, parent bank
	  hashes, bank signature counts, blockhash bytes, and optional hard-fork hash
	  data before deriving Solana residual, branch, or bank-state transcripts.
	  The lower-level Solana Tower lockout/replay, bank-fork, and AccountsLtHash
	  recursive public-input helpers now reject duplicate aliases for finalized
	  slots, epochs, rooted/parent slots, parent-bank hashes, bank hashes,
	  bank-fork hashes, Tower vote slots, transaction-status roots,
	  account-inclusion roots, AccountsLtHash checksum/root fields, full
	  AccountsLtHash bytes, and hard-fork data before hashing.
	  Their direct v1 Solana finality-context canonicalizers now apply the same
	  strict alias guard to portal-supplied context objects before hashing,
	  covering Tower vote slots, parent-bank hashes, bank signature counts,
	  optional hard-fork data, AccountsLtHash roots/checksums, stake roots, and
	  Tower replay/bank-fork transcript hashes.
	  JavaScript and Python TRON receipt, receipt-state, and transaction-source
	  proof helpers now reject duplicate aliases for source event digests,
	  receipt/message roots, transaction roots, transaction indexes/counts/bytes,
	  transaction Merkle branches, receipt-MPT proof nodes, optional expected
	  bridge emitter/owner addresses, and inclusion branches before deriving TRON
	  source proof hashes.
	  Their TRON raw block-header, solid-block header proof, solid-block message,
	  witness-schedule payload, witness-seal, and witness-schedule transition
	  helpers apply the same guard to block ids, raw-data hashes, header
	  roots/signatures, witness rosters/weights, signers bitmaps, transition
	  epochs, transition block hashes, schedule hashes/payload hashes, nested
	  seal proofs, and transition message hashes before deriving TRON
	  source-finality evidence.
	  JavaScript and Python Substrate storage-proof, runtime-storage request,
	  authority-set payload, authority transition, GRANDPA justification, and
	  transition-justification helpers now reject duplicate aliases for source
	  domains, source event indexes, finalized block fields, GRANDPA set ids,
	  storage roots, authority rosters/weights, payload hashes, transition
	  hashes, signers bitmaps, nested verifier material, and runtime storage
	  proof hashes before deriving Substrate source-proof or OpenVerify request
	  material.
  JavaScript, Python, Kotlin, and Java Android prover callbacks now pass defensive
  request snapshots into app-linked proof engines, and the Kotlin/JVM plus Java
  Android final-proof regressions now pin actual snapshot delivery for Solana,
  TON, EVM-family, TRON, and Substrate proof engines. Swift now mirrors that
  defensive snapshot path for Solana, TON, EVM, TRON, and Substrate final-proof
	  engines plus the Solana source-state proof engines. Kotlin Solana final-proof
	  witness objects now also defensively copy AccountsLtHash, bank hard-fork data,
	  and inclusion-branch byte buffers on construction and access, so a mobile UI
	  prover cannot mutate request witness bytes while proof-result wrapping still
	  uses the original canonical request. The Kotlin Solana prover also snapshots
	  raw witness input before app-controlled witness-provider resolution, preventing
	  resolver-side mutation of caller-owned AccountsLtHash or inclusion-branch
	  buffers before the canonical proof request is built; Java Android Solana
	  now also passes a distinct defensive `WitnessInput` snapshot to witness
	  providers. Kotlin/JVM and Java Android EVM-family, TON, TRON, and
	  Substrate-family facades now pass copied bundle/source-proof byte arrays
	  into app-controlled witness providers before canonical request construction.
	  Swift/iOS now routes Solana, TON, EVM-family, TRON, and Substrate-family
	  witness-provider calls through explicit input snapshot helpers as well.
	  JavaScript and Python portal facades now apply the same mutable
	  deep-snapshot boundary before invoking witness providers, so resolver-side
	  edits to nested public-input objects or byte buffers cannot alter
	  caller-owned UI state.
	  Swift, Kotlin, and Java Android Solana source-state facades also validate
	  canonical AccountsLtHash and role-separated full-light audit requests before
	  invoking app-linked proof callbacks, so malformed OpenVerify/FastPQ transcript
	  bytes cannot reach mobile proof engines and then be rejected only after proof
	  generation. JavaScript, Python, Swift, Kotlin, and Java Android direct
	  request builders plus source-state wrappers now also reject Solana
	  full-light audit requests whose role verifier hash reuses the request-bound
	  source-state, material, deployment, gate, finality, vote-message, nested
	  AccountsLtHash proof, or audit-statement hashes before any UI-generated
	  proof bytes are requested or wrapped.
	  Swift, Kotlin, and Java Android TON source-state facades now apply the same
	  pre-callback request validation to direct shard-state and role-separated
	  full-light audit requests, so tampered TON FastPQ metadata, statement bytes,
	  public-input columns, or verifier material cannot reach user-facing mobile
	  proof engines. The Swift TON facade also passes copied callback snapshots
	  into linked final-proof, shard-state, and audit proof engines, matching the
	  hardened Solana and JVM/Android callback surfaces before wrapping
	  UI-generated proof bytes against the original canonical request.
	  Web/Python source-state and destination proof-result wrappers now reject
	  padded or mismatched `proofBase64`, `proofFamily`, circuit id, request echo,
  structured public-input, proof-context, and Groth16 public-signal metadata
  before wrapping UI-generated proofs; duplicate camelCase/snake_case result
  aliases for those fields fail instead of letting one value be displayed while
  another is submitted. JavaScript and Python source-state result metadata now
  compares normalized numeric slots, role codes, canonical hex hashes, and
  Solana audit-role aliases against the request while still rejecting padded
  plain string metadata. Their canonical Solana/TON source-state proof capsule
  parsers now apply the same duplicate-alias rule to proof version, proof
  family, circuit id, proof bytes, and proof base64 before hashing source proof
  capsules. JavaScript and Python Solana final proof-result and submission
  builders now also apply that alias guard to wrapped proof bytes,
  proof-context/envelope/deployment hashes, source-state verifier echoes, and
  nested source-proof public-input fields before deriving wallet/RPC packages.
  JavaScript and Python Solana final proof-request builders now reject duplicate
  witness and nested proof-context aliases before request hashing or app-linked
  prover invocation, covering slots, bank-state hashes, blockhash spellings,
  message ids, deployment material, source-state verifier metadata,
  AccountsLtHash fields, and inclusion branches. Their Solana AccountsLtHash
  source-state and role-separated full-light audit request builders now apply
  the same duplicate-alias guard before deriving FastPQ statement/context/schema
  bytes, vote-message hashes, finality-context fields, source
  material/deployment selectors, or full-light gate/material/deployment hash
  echoes for browser and portal-backend provers.
  The Rust `iroha_sccp` finalized-vote verifier regression now also rejects a
  re-signed Solana finality context whose
  `accounts_lt_hash_proof_public_inputs_hash` no longer matches the recomputed
  AccountsLtHash public-input transcript.
  Their generic source-verifier material and source-adapter deployment
  normalizers now also reject duplicate aliases across source-domain,
  verifier-hash, source-bridge, target-domain, adapter verifier-key, Solana/TON
  audit-role, and deployment-receipt fields before deriving governed material
  or deployment record hashes, and explicit null audit-role hashes now fail
  instead of being treated as omitted zero-hash fields.
  JavaScript, Python, Swift, Kotlin, and Java Android source-material helpers
  now also reject non-zero lane-inapplicable source-state, bridge-emitter, and
  bridge-config fields before hashing, while still accepting canonical zero or
  empty fields emitted by normalized records.
  JavaScript and Python TON submission builders apply the same guard while
  packaging wrapped proof results into wallet/liteserver message-body BOCs,
  including top-level `proofResult`, proof bytes, request/envelope/deployment
  hashes, verifier echoes, proof context, bundle/source proof bytes,
  statement/destination hashes, metadata bytes, and `queryId`. Those builders
  now require the wrapped UI prover result before BOC construction, and the
  Swift/iOS, Kotlin/JVM, and Java Android message-body input types mirror that
  requirement instead of exposing standalone raw proof-byte submission
  constructors. Raw native-recursive proof bytes can no longer bypass request,
  verifier, or source-adapter deployment binding checks in web/backend or
  mobile wallet packaging. Their TON
  submission metadata canonicalizers now reject duplicate aliases across
  manifest fields, destination binding hashes, public inputs, and statement
  hashes before metadata bytes are hashed into those BOCs.
  Swift Substrate now surfaces the same field-specific
  base64 rejection. Web/Python and Swift/Kotlin/Java Android Substrate
  authority-transition builders also bind signer bitmaps to exact signer
  counts, signed and total weights, and a strict `> 2/3` quorum before hashing
  UI witness material, while web/mobile TON validator-signature transcripts
  bind validator public keys, signer bitmaps, total/signed weights, strict
  `> 2/3` quorum, and non-zero 64-byte signatures across the dedicated TON
  prover path plus the shared Kotlin/Java Android source-proof facade. Python
  and JavaScript TON shard-state request builders now also recompute nested
  validator-set `transitionSignatureHash` values before the transition chain is
  hashed, matching the Swift/Kotlin/Java Android TON prover path. TON
  source-state proof wrappers across web/mobile now recompute statement-byte
  hashes plus FastPQ `dsid`/`txSetHash` before accepting user-prover output, so
  callback metadata cannot drift from the request bytes sent to the prover.
  JavaScript and Python TON shard-state source proof input normalizers now
  reject duplicate aliases across masterchain/shard coordinates, BoC proof
  openings, config-proof BoC sources, verifier material, and finality metadata
  before deriving FastPQ public inputs or UI prover request hashes.
  Their raw TON shard proof transcript builders now apply the same duplicate
  alias rule to source-event digests, masterchain/finality aliases, shard
  transaction fields, dictionary openings, and inclusion branches before
  hashing branch witness material.
  Their TON full-light audit request builders also reject duplicate
  source-verifier material, source-adapter deployment, flattened/nested
  masterchain-config witness, shard-state public-input, and verification-proof
  hash aliases before deriving role-separated audit statements, OpenVerify
  columns, FastPQ metadata, or prover requests.
  Their TON validator-set, masterchain config, block-message,
  validator-signature, transition-message, and transition-signature transcript
  builders now also reject duplicate aliases across validator rosters, weights,
  signer bitmaps, quorum weights, config proof fields, masterchain/shard block
  coordinates, and validator-set transition payload hashes before hashing
  trust-anchor witness material.
  Python and JavaScript TON source-state wrappers also reject duplicate
  camelCase/snake_case request aliases, including nested FastPQ aliases, so UI
  proof requests cannot display one field spelling while hashing another. The
  same web/Python TON final proof-result wrappers now reject duplicate result
  aliases and recheck optional public-input, proof-context, statement,
  destination-binding, source-state verifier, and deployment-binding echoes
  before wrapping recursive proof bytes for submission. Web/Python TON final
  proof-request builders now apply the same alias guard before request-hash
  derivation and require direct/nested proof-context destination binding hashes
  plus top-level/nested source-adapter deployment hashes to agree. Swift,
  Kotlin/JVM, and Java Android TON proof-request inputs now accept the typed
  source-adapter deployment binding directly and enforce TON -> SORA binding
  domains before hashing mobile prover requests. Swift,
  Kotlin, and Java Android BSC ValidatorSet metadata builders now reject
  omitted or oversized account/storage MPT proof vectors and non-20-byte
  ValidatorSet contract addresses before hashing UI-submitted transition
  metadata; Python, JavaScript, Swift, Kotlin, and Java Android also recompute
  BSC storage-value hashes from the opened storage bytes so displayed metadata
  cannot diverge from the values submitted on-chain. Rust commit-seal transcript
  hashing now uses the same BSC validator-set, signer-bitmap, recovered-address,
  total/signed-power, and strict `> 2/3` quorum checks as the verifier; Python,
  JavaScript plus `dist`, Swift, Kotlin, and Java Android expose matching
  BSC commit-message and commit-seal helpers so portal and mobile UI provers
  derive the seal hash locally before submitting source proofs on-chain.
  The JavaScript TypeScript declarations now publish a shared
  `SccpDomainIdInput` for SCCP source, target, local, and counterparty domain
  request fields plus `SccpVersionInput` for v1-only proof/request inputs,
  including source-state prover `proofVersion` aliases. Package declaration
  tests pin those domain/version signatures plus the BSC commit-message and
  commit-seal inputs, including camelCase/snake_case aliases and canonical
  decimal string/bigint numeric forms, so TypeScript portal code can call the
  same proof-generation helpers that runtime validation accepts.
  Rust core plus web/mobile ETH/BSC sync-committee builders now require exact
  BLS public-key, proof-of-possession, and aggregate-signature widths, reject
  all-zero committee or aggregate material, bind signer bitmaps and padding to
  the committee size, bind total/signed weights, and require strict `> 2/3`
  quorum before hashing UI witness material or deriving aggregate transcript
  hashes.
  The JavaScript package root now re-exports the same helpers as the `./sccp`
  subpath, so
  published portal bundles can use the normal package entrypoint. Swift,
  Kotlin, and Java Android now also use the same Rust/Python/JavaScript
  destination-binding key format for EVM/TRON rollout metadata, with the
  network-id segment rendered as raw lowercase bytes32 hex while normalized
  returned `networkId` fields keep their `0x` prefix. TRON Rust destination
  binding, Swift Torii raw-submit, and operator evidence helpers now reject
  whitespace-padded Base58Check verifier addresses instead of normalizing
  them into governed SORA -> TRON binding metadata or UI prover request
  hashes. The reference
  TRON source-bridge evidence helper now rejects embedded whitespace inside
  inline or file-backed runtime bytecode, fixed-width deployment hashes, and
  hex-form TRON addresses, plus uppercase hex or `0X` aliases for those exact
  fields, before Python `bytes.fromhex()` can normalize the value into shorter
  material, keeping direct TOML generation exact. TRON live source-event
  readback now applies the same exact lowercase hex policy to log addresses,
  empty log data, and visible `TriggerSmartContract` calldata, rejects uppercase
  or `0X` aliases for exact transaction, signature, block-header, and
  route-canary hex, plus generic result-extension hex used while reconstructing
  unrelated block transactions, before transaction evidence can be accepted. The reference
  secp256k1 verifier now
	  also rejects non-canonical attestation ABI, zero native-proof hashes, zero
	  statement/public-input fields, and mismatched message/commitment public
  inputs before signature recovery. Direct/live ETH/BSC source and destination
  rollout TOML now carry replayable source-bridge, bridge-wrapper, and
  verifier runtime bytecode plus the canonical EVM backend and proof-family
  hash comments, and the all-lanes preflight rejects missing or drifting values
  before activation. EVM-family live JSON-RPC reads now require lowercase
  `0x` data and shortest-form quantities before runtime bytecode, route canary
  logs, transaction calldata, or deployment receipts can feed proof evidence;
  source and destination live reads also bound successful response and HTTP
  error bodies and reject duplicate JSON keys before decoding.
  Direct ETH/BSC source receipt metadata now also requires exact positive
  integer deployment block numbers, so boolean placeholders cannot make
  mined-receipt evidence look production-ready. Direct ETH/BSC
  source evidence helpers now also require exact `u32` source/target domain ids
  before verifier-key, material, or deployment hashes are derived, so
  `target_domain = False` cannot stage SORA-bound source evidence. They also
  reject padded bridge addresses, component hashes, domains, and deployment
  block numbers before deriving source material or deployment-record hashes.
  Solana, TON, and Substrate-family source-state evidence helpers now apply the
  same exact-input posture to fixed-width component hashes, source/target
  domains, and Substrate runtime-lane selectors before rendering source
  material, source-adapter deployment records, or full-light-client gate hashes.
  The
  Source, destination, live, and all-lanes evidence helpers now also require
  canonical ASCII decimal text for source-domain fields, EVM live/source-live
  RPC chain ids, deployment block numbers, audited ProgramData slots, TON
  workchain ids and last-transaction logical times, runtime version fields,
  and fallback all-lanes TOML integers, so non-ASCII digits, leading-zero
  values, hex forms, or signed forms cannot drift from reviewed operator
  evidence. EVM live and source-live collectors now also reject whitespace-padded
  JSON-RPC quantities and hex byte strings before rendering runtime bytecode,
  receipt, or rollout metadata. Solana/Substrate live collectors now also
  reject padded ProgramData executable base64, Substrate `specName`, and
  finalized runtime `:code` hex before rendering production TOML metadata.
  Solana, TON, and Substrate destination/live evidence helpers now also reject
  non-canonical base64 pad-bit aliases for verifier program bytes, Solana
  JSON-RPC account data, ProgramData metadata, TON code BoCs, and finalized
  runtime code before TOML rendering or all-lanes preflight can normalize
  copied evidence. TON code BoC text files now also reject internal whitespace
  instead of joining it into deployable code evidence, and the TON live
  collector returns accepted remote code BoCs as canonical standard base64.
  EVM destination/source, Solana, TON, TRON, and Substrate-family live
  collectors now bound successful HTTP response bodies and HTTP error details
  before decoding, and reject duplicate keys in remote JSON objects so live
  evidence cannot depend on last-value-wins parsing. TON and TRON runtime API
  keys must be exact non-empty ASCII tokens without whitespace or control
  characters; file-backed keys may only carry terminal newlines.
  The all-lanes activation preflight now also rejects padded fixed-width
  structured hashes, hash comments, route allowlist hashes, and route canary
  hashes plus duplicate known metadata comments before final production
  readiness can be reported; chain-specific metadata comment aliases that map
  to the same internal field also fail instead of overwriting earlier reviewed
  values. When both real
  `route_canary_*` config fields and imported canary metadata comments are
  present, the all-lanes gate now requires exact agreement so a direct
  `passed` value cannot override contradictory imported evidence.
  Solana, TON, and Substrate live destination wrappers now also require literal
  boolean readiness from the canonical destination summaries, so truthy strings
  cannot unlock offline TOML hashes. TON and Substrate live wrappers revalidate
  direct caller-supplied live metadata before deriving destination args, so
  forged account status, BoC hash-match flags, runtime-code metadata, verifier
  entrypoints, or hash algorithm labels fail before TOML readiness. EVM live
  destination TOML rendering now also recomputes imported summary bytecode
  hashes, backend/proof-family hashes, binding hashes/keys, domain metadata,
  canonical RPC chain ids, and expected-pin metadata before rendering. EVM
  source-live TOML rendering now also recomputes imported source bridge
  bytecode hashes, receipt deployment metadata, source record hashes, canonical
  ETH/BSC RPC chain ids, and expected-pin metadata before rendering. The
  JavaScript proof-request and payload/submission helper runtime rejects
  boolean domain values instead of
  coercing them into SORA/ETH domain ids, and its shared SCCP hex parser now
  rejects surrounding whitespace before TRON proof-request hashes,
  source-event digests, destination binding hashes, or proof transcript fields
  are normalized. Swift TRON proof helpers now mirror that exact fixed-width
  hex policy before request hashes, public signal words, proof envelopes, or
  verifier-call calldata are derived. Python SDK shared hex parsing now applies
  the same exact inline policy before TRON proof-request payload/statement
  hashes, receipt/source-event transcripts, destination binding material, and
  raw-header fields are normalized. Python Torii typed artifact, proof-job,
  bridge-proof, and bridge-message helpers now also reject padded TRON
  Base58Check verifier addresses, padded normalized TRON codec payloads, and
  surrounding/internal whitespace in deployment or proof hex before network I/O.
  JavaScript, Swift, Kotlin, and Java Android Torii client preflights now match
  that exact-input rule for TRON verifier addresses and deployment/proof hex
  while preserving byte-array proof inputs. Kotlin and Java Android TRON mobile
  prover request builders now also reject padded fixed-width payload and
  statement hashes before proof requests or envelopes are derived.
  JavaScript and Python SDK domain normalizers now also reject non-canonical
  string/number spellings such as `05`, `0x5`, `+5`, whitespace-padded text, or
  floats before any SCCP request or transcript hash can bind the lane id, and
  the shared dynamic unsigned-integer normalizers and Kotlin/Java Android
  string-based SCCP source-hash helpers plus TRON mobile prover request
  builders now apply the same exact integer/canonical-decimal policy before
  block numbers, slots, weights, indexes, finality heights, or proof public
  inputs are hashed. Swift, Kotlin, and Java Android source-proof helpers now also
  reject padded fixed-width TRON source-event and raw-header hex before
  transcript hashing. TRON live source-event readback now also rejects padded
  transaction ids, raw transaction bytes, signatures, event log addresses,
  topics, block transaction ids, trigger calldata, constant-call ABI words,
  `/wallet/getcontract` runtime bytecode, and internal whitespace inside hex
  payloads, and it applies the same exactness to source-event transaction
  `raw_data_hex` and source-proof `Result` bytes plus live block-header
  `blockID`, `txTrieRoot`, `parentHash`, `witness_address`,
  `witness_signature`, and `accountStateRoot` fields plus non-canonical high-S
  recoverable secp256k1 signatures before a transaction or deployment readback
  can be treated as replayable source-event evidence. Saved route-canary
  full-TOML replay now also reparses the carried `raw_data_hex` and raw
  recoverable signature, requiring the canonical low-S form before accepting
  owner, selector, proof-header, and signature-recovery metadata.
  Operator-supplied TRON source-event proof inputs now also reject padded
  digests, receipt roots, inclusion branches, witness schedule payloads,
  witness-seal bitmaps, non-canonical witness-seal signatures, and expected
  proof hashes before live proof material is summarized. All-lanes
  TRON ingestion preserves that exactness, including lowercase fixed-width hex,
  for live metadata comments and structured source/destination hashes before
  recomputing source material, destination bindings, or route readiness. Direct EVM-family
  source/destination evidence helpers now also reject padded inline runtime
  bytecode arguments before deriving deployment code hashes, while runtime
  bytecode files remain tolerant of ordinary file whitespace such as newlines.
  JavaScript, Python, Swift, Kotlin, and Java Android proof-request builders
  also reject non-empty all-zero optional source proof bytes before request
  hashing while preserving absent source proofs through diagnostic request
  hashes, app-linked prover calls, proof-result wrappers, and EVM/TRON/TON
  submission constructors, so portal/mobile proof UIs can submit externally
	  generated proof packages without fabricated source-chain witness bytes.
		  The JavaScript TON portal request builder now presence-checks
		  `sourceProofBytes` like EVM, TRON, and Substrate, so falsey non-byte values
		  cannot silently become an omitted source proof before request hashing, and
		  TON submission metadata bytes use the same presence check before BOC
		  packaging. The JavaScript TON and Solana submission builders now also
		  reject explicit non-object nested proof contexts, and the TON request
		  builder rejects explicit non-object source-adapter deployment bindings
		  before hashing or BOC packaging.
	  Python TON submission packaging now mirrors that presence-aware treatment
	  for proof-result statement hashes, destination-binding hashes, proof
	  context, and local BOC cell serialization, rejecting explicit falsey
	  values instead of falling through to defaults, nested proof context, or
	  empty cell fields.
	  Python EVM-family, TRON, and Substrate-family request builders plus
	  EVM/TRON destination-binding helpers now apply the same rule to
	  backend/proof-family/context inputs, and Solana/TON source-state verifier
	  defaults plus Solana genesis defaults no longer mask explicit falsey UI
	  values before request or witness hashing.
	  Python Groth16 public-signal derivation, Solana blockhash/witness context,
	  TON proof-context/deployment-binding, and TON submission metadata parsing
	  now also reject explicit falsey nested inputs rather than replacing them
	  with adjacent top-level fields or defaults.
	  Python Solana submission packaging now treats explicit empty proof/context
	  overrides as invalid input instead of falling back to wrapped proof-result
  fields, and Python source-state proof capsule/deployment normalization no
  longer promotes explicit zero versions or empty proof-family fields to the
  production defaults. Python SCCP transcript builders now also reject explicit
  zero versions instead of silently promoting them to production `v1`, and the
  JavaScript web SDK now applies the same v1-only preflight before deriving
  source transcript bytes for portal-generated proofs. Swift, Kotlin, and Java
  Android now mirror that first-release policy for public source-state proof
  capsules plus ETH/BSC/TRON/TON/Substrate-family transcript builders used by
  mobile prover UIs. JavaScript, Python, and Java Android Solana source-state
  proof capsules now also reject explicit null proof-version/proof-family
  metadata instead of promoting it to production defaults. JavaScript and
  Python source-state capsule normalizers also reject supplied `proofBase64` /
  `proof_base64` text unless it matches the proof bytes before canonicalization
  or AccountsLtHash proof hashing, binding UI-visible proof text to the bytes
  submitted on-chain. JavaScript, Python, Swift, Kotlin, and Java Android
  Solana source-state proof capsules now also mirror Rust's 2 MiB source-state
  proof cap plus the 128-byte proof-family/circuit-id label cap before wrapping
  or canonical hashing, and the dynamic web/Python normalizers apply the byte
  cap before base64 comparison so oversized UI prover output fails without
  extra display encoding. JavaScript, Python, Swift, Kotlin, and Java Android
  Solana source-state wrappers now recompute the AccountsLtHash public-input
  hash or full-light audit statement hash from `statementBytes` and require
  FastPQ `dsid`/`txSetHash` to derive from that canonical statement before
  wrapping UI-generated proof bytes. JavaScript and Python Solana source-state
  request wrappers now also reject duplicate top-level, nested FastPQ
  public-input, and FastPQ transition aliases before proof wrapping, so portal
  displays cannot drift from the fields hashed into the source-state transcript.
  Swift, Kotlin, and Java Android AccountsLtHash and full-light audit request
  builders now also reject explicit witness/opened `accountsLtHash` mismatches
  and normalize absent witness values from the opened full-bank hash before
  canonical request construction. JavaScript and Python AccountsLtHash and
  full-light audit request builders now derive opened contribution/residual
  hashes from canonical normalized full-bank fields before request hashing, so
  supported alias spelling cannot change the portal/backend hash boundary while
  duplicate aliases remain rejected. Their Solana full-light audit builders now
  also require the completed nested AccountsLtHash proof capsule and recompute
  `accountsLtHashProofHash` from it, rejecting proof-hash-only second-stage
  request construction. The JavaScript TypeScript declarations now model that
  contract with a required `accountsLtHashProof`/`accounts_lt_hash_proof`
  alias union and keep `accountsLtHashProofHash` as an optional consistency
  echo instead of an alternate input.
  JavaScript and Python
  app-linked Solana source-state prover result parsers use the same ordering
  and require returned scalar request echoes, audit roles, structured
  public-input/FastPQ metadata, and proof-family/circuit-id metadata to be
  exact, unpadded request matches before checking optional returned base64
  metadata, and they reject duplicate camelCase/snake_case proof-byte or
  proof-base64 aliases in dynamic result maps. Rust native recursive proof
  packaging and transparent-proof structure checks now also cap
  Solana/TON/Substrate-family proof payloads at 2 MiB, and the JavaScript,
  Python, Swift, Kotlin, and Java Android Solana, TON, and Substrate-family
  proof-result/submission wrappers mirror that bound before deriving envelope
  hashes, accepting app-linked prover output, or packaging wallet/RPC payloads.
  Their default destination rollout blockers now track only missing live native
  verifier deployment and trust-anchor evidence, not stale relayer-wiring
  blockers for the already-modeled program instruction, TON internal-message,
  or Substrate runtime-call packages.
  JavaScript and Python EVM-family, TRON, and
  Substrate-family local prover callbacks now apply the same exact canonical
  check to optional returned `proofBase64` / `proof_base64` metadata before
  proof-result wrapping, so a browser proof UI or portal backend cannot display
  or forward stale base64 while submitting different proof bytes. Swift, Kotlin,
  and Java Android source-state capsule surfaces derive base64 from defensive
  proof-byte copies rather than accepting
  caller-supplied aliases for Solana and TON capsules, and tests now pin that
  returned byte views cannot mutate the capsule or derived base64. JavaScript
  and Python now apply the same
  omitted-vs-explicit-null distinction to public UI
  transcript version fields, require wrapped Solana proof-result
  context/deployment versions during submission packaging, and reject explicit
  null source-adapter `adapterProofFamily` metadata. Those web/Python proof
  request and deployment normalizers now also reject explicit null source or
  target domains instead of promoting them to lane defaults. The JavaScript
  TypeScript submission declarations now mirror runtime Solana packaging by
  requiring exactly one wrapped proof-result alias (`proofResult` or
  `proof_result`) for SORA -> Solana on-chain submissions.
  Dynamic JavaScript and Python submission builders for Solana, TON,
  EVM-family, and TRON now also keep omitted fields distinct from explicit
  null proof, public-input, proof-context, statement, destination-binding,
  proof-context-hash, public-signal, and bundle overrides before wallet
  instruction, BOC, or verifier calldata packaging. The Rust TON
  internal-message builder now applies the same submit-ready context gate
  directly, rejecting non-TON public inputs, non-`ton-contract-v1` manifests,
  mismatched destination bindings, zero statement hashes, and empty bundle bytes
  before a wallet BOC can be emitted. Rust now also exposes the same
  UI-prover TON proof request/result wrapper path as the web, Python, and
  mobile SDKs, binding request hashes, envelope hashes, source-state verifier
  material, and governed TON -> SORA source-adapter deployment bindings before
  proof-result-based BOC/submission packaging. A Rust golden vector now pins
  the TON public-input bytes, deployment-binding hash, request hash, and
  envelope hash against the Python SDK implementation, and matching
  JavaScript, Python, Swift/iOS, Kotlin/JVM, and Java Android SDK tests now pin
  that same vector so UI-prover transcript hashing cannot silently drift.
  JavaScript, Python, Swift/iOS, Kotlin/JVM, and Java Android now also expose
  the canonical TON live-account route-canary evidence bytes/hash used by Rust
  and operator evidence scripts, giving web portals, relay backends, and mobile
  apps the same pre-submit rollout transcript checks before user-generated
  proofs are sent on-chain.
  JavaScript and Python Substrate-family runtime prover callbacks now validate
  optional returned transparent public inputs, proof context, statement hash,
  and destination-binding hash before wrapping proof bytes. JavaScript, Python,
  Swift, Kotlin, and Java Android production proof wrappers preserve omitted
  source proof bytes through app-linked prover output and submission packaging,
  while rejecting non-empty all-zero placeholders.
	  JavaScript and Python local-prover facades now also accept plain async
	  witness-provider functions and `resolve_witness` objects in addition to
	  `resolveWitness`, and tests pin that browser/backend relay providers resolve
	  Solana, TON, TRON, Substrate-family, and EVM-family proof inputs from
	  snapshots before linked prover callbacks receive canonical requests; package
	  declaration tests pin the same witness-provider hooks for TypeScript portal
	  consumers. JavaScript now rejects duplicate hook aliases
	  (`witnessProvider`/`witness_provider`, `resolveWitness`/`resolve_witness`,
	  and `prove`/`proveFn`/`prove_fn`) before request construction, and the
	  TypeScript declarations model those hooks as exactly-one alias unions.
	  Python portal-backend witness-provider objects now reject duplicate
	  `resolve_witness`/`resolveWitness` methods before request construction too.
	  Swift/iOS, Kotlin/JVM, and Java Android prover tests pin the same UI-owned
	  ordering contract for mobile proof wrappers, with Kotlin/JVM and Java Android
	  also mutating provider-visible byte snapshots to prove caller-owned bundle
	  arrays remain unchanged.
  Public Kotlin/Java proof-result wrappers recheck TON/EVM/TRON backend ids
  before packaging proof bytes. Swift/iOS, Kotlin/JVM, and Java Android now
  mirror the Rust/JavaScript/Python BN254 G1/G2 curve and G2 prime-order
  subgroup preflight for EVM-family and TRON Groth16 proof bytes before mobile
  proof wrappers package UI-generated output. Java Android EVM-family and TRON
  proof-result records now also snapshot and freeze public-signal word lists,
  matching Kotlin's immutable mobile wrapper behavior so caller-side list
  mutation cannot change a wrapped UI proof package after construction. Python,
  JavaScript, Swift, Kotlin, and Java Android TRON
  transaction-source proof helpers now also recover the signer from
  `sha256(raw_data)` and require it to equal the source-call owner address
  before deriving source-proof bytes, matching Rust/core admission and keeping
  wrong-key-but-canonical source-call signatures out of portal/mobile
  transcripts. Python, JavaScript, Swift, Kotlin, and Java Android now also
  expose the TRON v3 transaction route-canary transcript helper, with shared
  vectors for the governed destination binding, route allowlist hash,
  source-message public inputs, block metadata, and recovered-owner evidence.
  Route allowlist activation now also consumes
  real `route_canary_*` config fields, and the runtime/ZK policy hash bind the
  post-deploy route canary evidence to the canonical route allowlist hash and
  destination binding hash before a lane can become production-ready. Core
  readiness also rejects route canary evidence that reuses the governed source
  material record hash or source-adapter deployment record hash, and all-lanes
  preflight rejects EVM/TRON transaction canary fields plus TON live-account
  canary hashes when they alias governed source, deployment, route, or
  destination hash roles. The
  EVM-family operator helpers and all-lanes preflight now require route canary
  evidence to be derived from a successful `MessageProofAccepted` transaction:
  the receipt log, submitted `submitSccpMessageProof` calldata, 384-byte proof
  tuple header, deployed binding/backend/family/network tuple, and
  `usedMessageProofs(messageId)` state must all agree before the ETH/BSC route
  canary hash is accepted. The canonical EVM canary transcript also commits
  proof ABI version `1`, the SORA proof source-domain word, and the ETH/BSC
  target-domain word before the commitment root, preventing proof-version or
  EVM-family lane replay. The direct renderer, public hash helper, and
  all-lanes preflight now also reject reuse across distinct EVM canary
  transcript hash roles, including transaction hash, calldata, message id,
  payload, statement, commitment, and finality block fields.
  Rust/Core/Torii configured readiness now carries the
  same `evm_route_canary_*` transcript fields, recomputes the EVM canary hash
  from configured ETH/BSC rollout material, and rejects generic EVM canary
  hashes before all-lanes launch; direct EVM TOML now emits those runtime config
  fields instead of leaving them only as comments. TRON route-canary readiness
  now also requires the transaction owner address, signature SHA-256, recovered
  TRON address, and positive owner-recovery flag from the verified
  TriggerSmartContract transaction before configured launch can pass. The
  SCCP source proof envelope now binds non-SORA
  source messages to source/target domains,
  proof plan, finality model, message id, payload hash, commitment root, and
  source-event digest before public inputs are derived. Source consensus proof
  material now also carries a plan-specific adapter proof variant for
  ETH/BSC/Solana/TON/TRON/Substrate-family lanes, so generic self-consistency
  blobs or stale witness substitutions cannot masquerade as another chain's
  proof shape. Each adapter statement is now additionally wrapped in a
  FastPQ/OpenVerify proof capsule so adapter metadata, public inputs, and proof
  public IO are cryptographically bound before lane readiness can be considered.
  Shared STARK/OpenVerify decoding now requires canonical Norito bytes for the
  outer envelope, nested STARK wrapper, and backend FastPQ proof, rejecting
  alternate compressed framings before metadata is trusted.
  Source consensus proofs also carry explicit trust-anchor/verifier evidence
  for the source anchor, consensus verifier, message-inclusion verifier,
  finality policy, adapter proof, adapter transcript, and adapter circuit, with
  the evidence hash included in the adapter OpenVerify statement. Those
  evidence records are now sourced from typed `SccpSourceVerifierMaterialV1`
  records; the built-in catalog is placeholder-only and cannot satisfy the
  production gate until real source-chain trust anchors and immutable verifier
  hashes replace it; flipping the placeholder flag or reusing any built-in
  placeholder component still fails closed. Explicit-material production helpers
  now verify source envelopes against caller-supplied material, but production
  readiness also requires an exact domain profile: today ETH mainnet, BSC
  mainnet, Solana mainnet-beta, TON mainnet masterchain/shard, TRON mainnet
  DPoS solid-block/transaction-source, and Substrate-family GRANDPA/event-storage source
  profiles can satisfy the material gate only with deployment-supplied
  component hashes, while generic ids and hashes remain fail-closed unless they
  match an exact profile and avoid template-derived hashes. The TRON mainnet
  message-inclusion profile id now explicitly names the governed
  transaction-source verifier instead of the legacy receipt-root-branch label,
  keeping Rust, portal, mobile, and evidence vectors aligned with the
  production adapter proof shape. The offline
  source-evidence regression suite now exercises every template-derived
  component field across ETH, BSC, Solana, TON, TRON, and Substrate-family direct
  record hashing plus governance TOML rendering, so one live component cannot
  hide another placeholder component in production material. The same direct
  evidence helpers reject all-zero production component, bridge, adapter
  verifier-key, and deployment receipt hashes instead of relying only on CLI
  parsing, and the Rust source-material constructors now reject all-zero or
  template-derived role hashes plus reused non-zero role hashes before
  returning deployment-shaped material.
  Solana and TON deployment-backed production readiness now
  additionally require complete governed full-light-client audit bundles before
  the source-adapter gate can open, and Solana/TON full-light-client audit hashes
  must be role-separated from each other and from existing source-adapter
  material and cannot reuse built-in template source-material component hashes.
  `zk.sccp_source_verifier_materials`,
  `zk.sccp_source_adapter_engine_deployments`,
  `zk.sccp_destination_rollouts`, and `zk.sccp_route_allowlists` now thread
  configured source-verifier material, matching source-adapter deployment
  receipts, destination rollout records, and governed route allowlists into
  on-chain bridge proof admission and the ZK consensus policy hash. Material
  alone can no longer open a non-SORA source lane: admission also requires an
  exact deployment record for the same domain/profile/circuit with matching
  SORA target domain, trust-anchor, consensus-verifier,
  message-inclusion-verifier, finality-policy hashes, a non-zero deployment
  receipt hash, and an `adapter_verifier_vk_hash` equal to the canonical
  lane-specific source-adapter verifier commitment and the OpenVerify `vk_hash`
  embedded in the user-submitted source proof. The ETH/BSC, Solana, TON, TRON,
  and Substrate-family source evidence hash helpers now apply the same
  source/deployment role-hash separation before returning canonical record
  hashes, keeping live collectors and programmatic governance tooling aligned
  with TOML rendering and all-lanes preflight.
  JavaScript, Python, Swift,
  Kotlin, and Java Android SDKs now derive the same canonical source-material
  and source-adapter deployment record bytes/hashes for portal and mobile proof
  UIs, and they reject reused non-zero source/deployment role hashes before
  request hashing or app-linked prover invocation. Solana full-light-client
  audit request builders across JavaScript, Python, Swift, Kotlin, and Java
  Android now derive governed source-material, source-adapter deployment, and
  audit gate hashes from component/deployment records, reject stale precomputed
  annotations, and require the witness deployment hash and deployment receipt
  to match the derived deployment record before user-side proof generation.
  TypeScript declarations expose the same flattened component hashes,
  annotation fields, and witness deployment hash/receipt inputs for strict web
  portal callers.
  Their UI/mobile
  source-adapter deployment-binding normalizers also reject a non-zero
  deployment hash that equals its deployment receipt hash, matching the core
  production evidence role-separation rule before user-generated proofs are
  submitted on-chain. The all-lanes preflight
  also parses the source record hash comments emitted by those evidence helpers
  and rejects missing or stale material/deployment hash annotations, so
  user-side provers can audit governed lane evidence before submitting on-chain
  proofs. The same SDKs
  also derive canonical native destination
  binding keys and hashes for SORA -> Solana, SORA -> TON, and SORA ->
  SORA Kusama/SORA Polkadot/SORA2 runtime lanes, aligning user-side proof
  requests with the destination rollout evidence helpers. For EVM-family and
  TRON source
  lanes, material
  and deployment records must also carry the same governed source-bridge
  emitter id, address, and non-zero runtime code hash; non-emitter source
  domains must leave those fields empty/zero. It also requires an exact
  destination rollout and an exact activated route allowlist whose policy hash
  is bound to the canonical source-material record hash, source-adapter
  deployment record hash, and destination binding hash for that lane. The
  all-lanes evidence preflight now additionally requires each route allowlist
  table to carry passed post-deploy canary metadata bound to the same route
  allowlist hash and destination binding hash, so a stale canary from another
  route cannot make a lane production-ready; the canary evidence hash must also
  be distinct from every advertised source material record hash, source-adapter
  deployment record hash, route allowlist hash, and destination binding hash,
  and unique across all advertised lanes. Cross-lane canary replay blockers now
  attach to the target lane summary as well as the bundle summary, so per-lane
  rollout automation cannot treat a globally rejected lane as production-ready.
  Core and Torii configured runtime all-lanes admission mirror the same global
  replay checks, and the lane-aware Rust route-canary builder refuses source
  record hash replay before config objects are minted.
	  EVM-family destination, ETH/BSC source, Solana, TON, and Substrate-family
	  reusable render/summary evidence APIs now run the same deployed bytecode,
	  program bytes, runtime code, or code BoC hash derivation as their CLI paths,
	  so portal backends and SDK automation cannot bypass byte/hash mismatch checks
	  by importing helper modules directly. TON production TOML and all-lanes
	  readiness now preserve that derivation as explicit code-BoC base64,
	  root-hash, and match metadata, and the all-lanes gate decodes the staged BoC
	  to recompute the TON representation root. A copied TON code hash without
	  replayable BoC evidence remains diagnostic. Substrate-family live and direct
	  destination evidence now preserves finalized runtime code as base64, and the
	  all-lanes gate decodes it to recompute the BLAKE2b-256 runtime code hash
	  before accepting SORA-family runtime rollouts.
  The EVM-family, Solana, TON, and
  Substrate-family live/offline destination evidence renderers now require that
  metadata for production TOML via `--route-canary-evidence-hash`, keeping
  operator TOML generation aligned with the stricter all-lanes launch gate. The
  direct destination and TRON full-lane renderers also reject route canary
  hashes that reuse any governed source material record hash, source-adapter
  deployment record hash, route allowlist hash, or destination binding hash
  before JSON summaries or production TOML are emitted. Solana and
  Substrate-family destination renderers now also recompute the supplied route
  canary hash from immutable ProgramData or finalized runtime metadata, and
  the all-lanes gate rejects generic non-zero canary hashes for those lanes.
  JavaScript, Python, Swift, Kotlin, and Java Android SDKs now expose the same
  Solana immutable ProgramData route-canary transcript and hash derivation, so
  web portals and mobile apps can verify governed lane evidence before their
  app-linked provers submit proofs on-chain. Those helpers now fail closed on
  non-canonical Solana destination bindings by default and reject explicit
  expected destination-binding hashes that would steer route-canary evidence
  away from the governed SORA -> Solana rollout. Rust SCCP regression coverage
  also pins the same canonical binding rule at destination-rollout readiness,
  route-canary hash derivation, and route-allowlist evidence derivation.
  The
  default
  production path remains closed on the placeholder catalog when no complete
  configured lane material is present, and configured bridge-proof admission
  now enforces the all-lanes-at-once launch policy across every advertised
  remote SCCP domain. The same all-lanes gate now also runs when Torii receives
  an explicit EVM/TRON deployment destination binding, so a single configured
  outgoing destination rollout cannot expose production artifact/job/submission
  packaging while any advertised remote lane is still missing governed evidence.
  A complete source material, source-adapter deployment, destination rollout,
  and route allowlist for one lane cannot open inbound production admission
  while any other advertised lane is still missing production-ready governed
  evidence. TRON source material and deployment
  records must additionally carry the same non-zero source bridge network id,
  governed owner address, and config hash derived from the deployed bridge
  address, network id, source/target domains, and owner, so a reused emitter
  address or bytecode hash cannot satisfy the source lane without the matching
  governed bridge configuration. The all-lanes evidence preflight now also
  rejects unsupported remote domains, unsupported `zk.*` evidence sections,
  malformed direct evidence sections, non-integer domain fields, and unexpected
  fields outside each section's exact evidence schema before lane matching,
  including JSON/TOML boolean values that would otherwise alias domain ids in
  Python. TRON transaction-source proofs for production
  material also require the authenticated `TriggerSmartContract.owner_address`
  to match that configured owner. TRON sender
  and recipient codec
  validation now also rejects the checksummed all-zero `0x41` address payload,
  keeping the account surface aligned with the non-zero witness/verifier/source
  bridge address gates. The EVM destination side now has a
  `SccpGroth16Bn254MessageVerifier` implementation for the
  `evm-groth16-bn254-v1` backend, and the wrapper binds deployments to the
  expected verifier bytecode hash plus the Groth16 verifier's immutable
  verifying-key hash. The wrapper now rejects empty backend/proof-family
  labels, zero network ids, zero target domains, same-domain deployments, zero
  statement hashes, zero required public-input fields, and target-domain words
  that do not match the governed lane before verifier dispatch. Rust/Torii EVM
  destination-binding helpers now require
  the same verifier code hash and, for Groth16, a non-zero verifier key hash
  before producing deployment-specific bindings, and Rust EVM Groth16 package
  construction plus verification now parse the deployment-binding key and
  recompute the canonical binding hash before accepting a supplied relay
  package; ETH/BSC default destination blockers now track only missing live
  verifier deployment and trust-anchor evidence, not a stale relayer-wiring
  blocker. The offline
  `scripts/sccp_evm_destination_evidence.py` helper now recomputes the
  SORA -> ETH/BSC EVM Groth16 destination binding hash from network id,
	  verifier address, bridge wrapper address, verifier code hash, and verifier
	  key hash, rejects boolean or non-`u32` programmatic domain ids,
	  rejects non-canonical direct-helper backend/proof-family labels,
	  and renders the governed destination rollout plus route allowlist TOML only
	  after `--expected-destination-binding-hash` and
	  `--route-canary-evidence-hash` are present. Its direct TOML and JSON helpers
	  also reject caller-supplied destination binding hashes that differ from the
	  canonical lane binding, report unpinned, bridge-runtime-hash-missing, or
	  canary-missing JSON as not TOML-ready, and require the governed route
	  allowlist hash to recompute from the canonical source-material record hash,
	  source-adapter deployment record hash, and SORA -> ETH/BSC destination
	  binding hash before emitting route summaries. The direct helper can now
	  derive the bridge wrapper runtime hash from bytecode and rejects mismatches
	  with a supplied `--bridge-code-hash`. Binding-only JSON now omits route
	  evidence until the expected
	  destination binding pin is supplied and matched.
		  The live collector carries the same check before producing offline TOML
		  arguments, requires the wrapper's `destinationBindingHash()` view to match
		  the recomputed immutable deployment inputs, rejects supplied route allowlist
		  evidence before the expected destination binding pin matches the live
			  deployment, and only then carries the route allowlist/source-record hashes
			  plus the route canary evidence hash in its diagnostic offline argument
		  bundle. It now also withholds Torii
		  artifact/job destination query fields until that explicit binding pin
		  matches and marks emitted query fields as requiring the prover-produced
		  `proof_bytes_hex`, so EVM live evidence cannot look package-ready without
		  the external Groth16 tuple. ETH/BSC source live TOML now also requires
		  a fetched deployment transaction receipt whose status is `0x1`, contract
		  address is present as a non-zero EVM address matching the governed source
		  bridge, block hash is non-zero, block number is positive, and
		  `transactionHash` echoes the operator-supplied deployment transaction;
		  the all-lanes preflight treats missing source receipt metadata as a
		  rollout blocker. EVM destination live TOML now carries the verifier
		  runtime code hash and verifier key hash observed from JSON-RPC, and the
		  all-lanes preflight requires those live comments to match the structured
		  verifier fields. Destination rollout comment metadata must also echo the
		  structured network-id, bridge-address, binding-key, and binding-hash fields
		  when both are present, so stale operator TOML comments cannot hide behind
		  canonical fields during all-lanes activation. Direct EVM destination TOML now
		  emits the same RPC chain-id, bridge runtime code hash, verifier runtime code
		  hash, and verifier key hash comments required by all-lanes, and includes
		  replayable bridge/verifier runtime-bytecode comments when bytecode is
		  supplied. The live wrapper now carries `eth_getCode` bytecode through the
		  shared offline renderer, and all-lanes decodes those comments to recompute
		  Keccak-256 before ETH/BSC destination rollout evidence can pass. Direct
		  ETH/BSC source TOML now requires audited deployment
		  transaction, receipt contract address, receipt block hash, and receipt block
		  number metadata plus source bridge runtime-bytecode preimages before
		  rendering, emits the same EVM source live comments required by all-lanes,
		  and the EVM source live wrapper suppresses duplicates after reusing the
		  direct renderer. The offline
  `scripts/sccp_tron_source_bridge_evidence.py` renderer now applies the same
  route allowlist evidence binding on the TRON full-rollout path: a supplied
  `--route-allowlist-hash` must recompute from the canonical TRON source
  material record hash, source-adapter deployment record hash, and SORA -> TRON
  destination binding hash before JSON, direct TOML, or live-rendered full TOML
  can be emitted, rejects padded fixed-width component hashes and network ids
  before those records are derived, and JSON route checks now require the
  expected destination binding pin as well. Direct and live full TOML now also require
  `--route-canary-evidence-hash`, aligning TRON rollout generation with the
  all-lanes canary gate. The live collector also rejects a queried destination verifier
  whose `networkId()` differs from the queried source bridge `networkId()` and
  now requires an explicit `--expected-destination-binding-hash` match before
  emitting live full-TOML rollout records. The direct TRON helper also now
  accepts only canonical ASCII decimal `u32` domain text on the CLI and exact
  Python `int` domain values in importable hash/calldata APIs, preventing
  boolean, hex, leading-zero, or signed spellings from aliasing production lane
  ids.
  Rust route-evidence helpers now derive an evidence-bound allowlist hash only
  from production-ready source material, matching source-adapter deployment,
  production destination rollout material, and coherent TRON network ids; replayed
  or internally incomplete lane components leave route evidence unbound.
  The all-lanes evidence preflight mirrors this by refusing to recompute route
  hashes unless source material/deployment record hashes and the destination
  binding hash are present and non-zero.
  Core bridge-proof admission now has TRON regressions proving exact configured
  source material, source-adapter deployment, destination rollout, and
  route-allowlist evidence reaches the all-lanes launch gate, while a replayed
  route allowlist hash and a production-shaped destination rollout with a
  mismatched TRON network id are rejected after that source-adapter gate opens.
  Production still needs the real
  recursive SCCP circuit verifying key and governed deployment material before
  routes can be marked ready. The offline
	  destination evidence helpers now validate the deployed verifier identity and
	  non-zero code material before rendering exact SORA -> counterparty
	  destination rollout plus route allowlist TOML. Production TOML now requires a matching
	  `--expected-destination-binding-hash` and a non-zero
	  `--route-canary-evidence-hash`; unpinned or canary-missing JSON remains
	  diagnostic and is reported as not TOML-ready. Their direct TOML and JSON helpers
  reject mismatched caller-supplied binding hashes and require the route
  allowlist hash to bind the source material record hash, source-adapter
  deployment record hash, and SORA -> Solana destination binding hash. The live
  `scripts/sccp_solana_live_evidence.py` helper now collects the deployed
  Solana verifier ProgramData through read-only JSON-RPC, rejects mutable
  upgrade-authority programs and non-canonical Program account layouts, derives
  the verifier code hash as BLAKE2b-256
	  over ProgramData executable bytes, preserves those executable bytes as base64
	  in live summaries, offline replay arguments, and TOML metadata, and requires
	  pinned ProgramData plus code
	  hash values before rendering production TOML, including a positive
	  `--expected-programdata-slot` that must match the live ProgramData account
	  and the same route canary evidence hash required by all-lanes preflight.
	  That route canary hash is now recomputed from the governed route tuple,
	  verifier program id/code hash, finalized RPC commitment, immutable
	  ProgramData account metadata, read context slots, and deployed executable
	  bytes before direct/live Solana TOML or all-lanes readiness can pass. It
	  also rejects verifier code hash reuse across route allowlist, destination
	  binding, source material, and source deployment roles before the route
	  canary transcript is accepted.
	  The direct Solana destination helper and its importable render/summary APIs
	  can also derive the verifier code hash from supplied program bytes and
	  reject mismatches with an explicit `--verifier-code-hash`, so offline review
	  no longer depends on a manually transcribed executable hash. Inline direct
	  Solana verifier program bytes are now exact evidence: padded
	  `--verifier-program-bytes-hex` or `--verifier-program-bytes-base64` values
	  fail instead of being normalized into executable preimages.
	  Direct Solana destination TOML now requires audited ProgramData address,
	  ProgramData slot, and finalized RPC context slots before rendering and emits
	  the same immutable ProgramData comments required by all-lanes, including the
	  canonical 36-byte upgradeable Program account length.
	  The live evidence now also records BPF upgradeable-loader ownership,
	  immutable-program status, and finalized JSON-RPC context slots for the
	  Program and ProgramData account reads, keeps `confirmed` reads
	  diagnostic-only, and rejects ProgramData reads whose context slot is older
	  than the ProgramData deployment slot or Program account reads whose context
	  slot is older than that same deployment slot. RPC context slots must be
	  positive integer JSON numbers; booleans are rejected before evidence is
	  summarized or rendered. The offline Solana destination helper applies the
	  same exact-integer rule to importable ProgramData slot and context-slot
	  arguments before deriving ProgramData metadata or reporting TOML readiness.
	  The live helper now also rejects padded ProgramData slot arguments and
	  executable base64 metadata before deriving immutable ProgramData comments.
	  Its JSON dry runs include
	  replayable offline evidence arguments and a deterministic TOML digest after
	  all live and governance pins match. The all-lanes evidence preflight now
	  requires finalized Solana live RPC commitment, BPF-loader ownership,
	  immutable-program status, ProgramData address, pinned positive slot,
	  positive RPC read context slots at or after the ProgramData deployment
	  slot, and executable BLAKE2b-256 plus base64 executable-preimage metadata
	  comments. It decodes that executable preimage to recompute the hash and rejects
	  offline/manual Solana destination records that lack that
	  immutable-deployment evidence.
	  Solana direct JSON now also surfaces route-allowlist, route-canary,
	  ProgramData metadata, executable-preimage, and `full_toml_ready`
	  readiness booleans. Complete route evidence without ProgramData pins now
	  remains diagnostic with `programdata_metadata_ready = false`; stale
	  ProgramData metadata still fails closed. Live JSON distinguishes
	  `destination_toml_ready` from the final finalized/pinned
	  `full_toml_ready` gate.
	  The offline `scripts/sccp_ton_destination_evidence.py` helper now validates
	  TON raw verifier contract addresses as basechain workchain `0` addresses
	  and can derive non-zero verifier code hashes from single-root TON code BoCs
	  before rendering exact SORA -> TON destination rollout plus route allowlist
	  TOML. TON raw addresses, fixed-width hashes, last-transaction logical-time
	  text, live remote hash strings, and live/imported code-BoC base64 now reject
	  surrounding whitespace before they can enter rollout evidence. Production
	  TOML now requires a matching `--expected-destination-binding-hash` and
	  `--route-canary-evidence-hash`; unpinned or canary-missing JSON remains
	  diagnostic and is reported as not TOML-ready. Its direct TOML and JSON
	  helpers now also require the governed route allowlist hash to recompute from
	  the TON source-material record hash, audited source-adapter deployment record
	  hash, and SORA -> TON destination binding hash before emitting records or
	  summaries. The live `scripts/sccp_ton_live_evidence.py` helper now
	  collects TON Center v3 account-state evidence for the deployed verifier
	  contract, requires an active account with code BOC, recomputes the code BOC
	  root hash against the returned code hash, pins code hash plus account-state
	  hash plus the same route canary evidence before production TOML, and emits
	  replayable offline evidence arguments plus a deterministic TOML digest
	  after all pins match. Its JSON dry-run now splits
	  `destination_toml_ready` from `full_toml_ready`, so rollout automation can
	  distinguish complete live destination/route evidence from the additional
	  independent code-hash and account-state pins required for production TOML.
	  Direct inline `--verifier-code-boc-hex` and
	  `--verifier-code-boc-base64` values now reject surrounding or embedded
	  whitespace instead of normalizing padded code-BoC preimages; file inputs
	  remain suitable for raw, hex, or base64 artifacts. Offline replay arguments
	  include the returned code BoC
	  so direct TOML generation can rederive and emit the same code-BoC root
	  evidence.
	  The all-lanes preflight now requires those live TON account-state values
	  as governed destination rollout fields, keeps the imported comments in
	  agreement with the config fields when present, and decodes the staged
	  verifier code BoC plus the required live base64/hash-match comments to
	  recompute the TON representation root before launch readiness can pass.
	  Direct TON destination TOML now requires explicit
	  active account status, audited account-state hash, last transaction LT,
	  last transaction hash, and matching code-BoC bytes/root metadata; it emits
	  both replay comments and runtime `ton_*` rollout fields, while
	  offline/manual TON destination records that lack that status, audit, or BoC
	  replay evidence remain diagnostic and do not pass launch readiness.
	  TON route allowlists now also carry `ton_route_canary_*` live-account
	  snapshot fields. Runtime lane readiness and the all-lanes preflight both
	  recompute the route canary hash from the governed route hash, destination
	  binding hash, source material/deployment hashes, verifier identity/code
	  hash, active account status, account-state hash, last transaction LT/hash,
	  and code-BoC root hash, so a generic non-zero canary hash or drifted
	  live-account metadata cannot open the SORA -> TON lane. Direct TON
	  evidence and all-lanes validation also reject reuse between the live
	  account-state hash and last-transaction hash snapshot roles.
	  The offline `scripts/sccp_substrate_destination_evidence.py` helper now
	  renders exact SORA -> SORA Kusama/SORA Polkadot/SORA2 runtime destination
	  rollout plus route allowlist TOML with the fixed
	  `SccpBridge.submit_message_proof` verifier entrypoint. Production TOML now
	  requires a matching `--expected-destination-binding-hash` and
	  `--route-canary-evidence-hash` for the selected runtime lane; unpinned or
	  canary-missing JSON remains diagnostic and is reported as not TOML-ready.
	  It now rejects padded runtime-lane selectors and runtime `specName` values
	  before destination rollout or route metadata can be rendered. Its
	  direct TOML and JSON helpers reject mismatched caller-supplied binding hashes
		  and revalidate the fixed entrypoint and deployment code hash, then require
		  the route allowlist hash to bind the source material record hash,
		  source-adapter deployment record hash, and selected SORA ->
		  Substrate-family destination binding hash, with the three governed
		  hash roles required to be non-zero and pairwise distinct before the
		  transcript is accepted. Public release-bundle verification now
		  recomputes that route-allowlist transcript from embedded all-lanes
		  evidence instead of trusting the self-reported expected-hash match
		  flag. Direct Substrate-family
		  destination evidence can derive the runtime verifier code hash from supplied
		  runtime bytes and rejects mismatches with an explicit
		  `--verifier-code-hash`, matching the live finalized `:code` hash
		  derivation used for production evidence. Inline
		  `--runtime-code-hex` and `--runtime-code-base64` values now reject
		  surrounding or embedded whitespace instead of normalizing padded
		  runtime-code preimages.
		  Direct Substrate-family
		  destination TOML now also requires audited finalized head, runtime spec
	  name/version, and transaction version metadata, rejects runtime `specName`
	  values that do not match the selected destination lane, rejects boolean
	  runtime version placeholders before readiness is derived, and emits the
	  same runtime comments required by all-lanes. The live
	  `scripts/sccp_substrate_live_evidence.py` helper now collects finalized
	  runtime evidence from read-only Substrate JSON-RPC, pins the finalized
	  head, runtime spec/version fields, and BLAKE2b-256 hash of finalized
	  `:code`, requires the live `specName` to match the selected destination
	  domain, requires the same route canary evidence before production TOML, and
	  rejects padded `specName`, expected `specName`, runtime version text,
	  non-lowercase or non-`0x` finalized-head hex, and runtime `:code` hex
	  before emitting live metadata
	  comments required by the all-lanes preflight before
	  Substrate-family destination records can pass launch readiness. The all-lanes
	  preflight also rejects Substrate runtime comments whose `specName` belongs
	  to another runtime lane, and it now recomputes the Substrate-family route
	  canary hash from the governed route tuple, runtime entrypoint/code hash,
	  finalized head, runtime version metadata, and finalized runtime bytes
	  before accepting SORA-family runtime readiness. It also rejects runtime code
	  hash reuse across route allowlist, destination binding, source material,
	  and source deployment roles before the route canary transcript is accepted.
	  Configured Rust readiness
	  now carries the same finalized runtime fields in destination rollouts and
	  rejects SORA-family launch without them. The offline
  `scripts/sccp_substrate_source_evidence.py` helper now renders exact
  Substrate-family source material and source-adapter deployment TOML for the
  same three runtime lanes from governed GRANDPA/event-storage component
  hashes, adapter verifier key hashes, and deployment receipt hashes, and it
  rejects padded runtime-lane selectors, component hashes, and target domains
  before those record hashes are derived.
  Destination rollout records are now bound
  to domain, chain,
  exact mainnet/runtime anchor id, chain-specific verifier identity format, and
  a non-zero Groth16 verifier-key hash for EVM-family/TRON lanes before they can
  satisfy the production gate, while native Solana/TON/Substrate-family
  rollout records now reject any unexpected verifier-key hash. ETH/BSC require
  non-zero EVM contract addresses and reject verifier/bridge wrapper address
  aliasing across direct, live, and all-lanes evidence, Solana requires a
  non-zero program id, TON requires a non-zero raw contract address, TRON
  requires a checksummed base58 contract address, and Substrate-family lanes
  require the exact SCCP runtime entrypoint.
  EVM and TRON Groth16 relay packages are
  signer-free: they carry the verifier proof ABI tuple directly and reject
  attempts to use the reference attestation/signer path for the production
  backend, including verifier-side rejection when a submitted package reuses
  the generic manifest destination-binding hash. The normalized proof-job
  builder now has explicit signer-free Groth16 paths, so production EVM/BSC
  proof tooling must provide the Groth16 proof bytes and deployment binding
  instead of falling back to Torii signer
  attestations, and production TRON tooling must provide the TVM Groth16 proof
  bytes plus a deployment binding derived from the checksummed verifier
  contract address, verifier code hash, and verifier key hash instead of
  falling back to generic FastPQ/OpenVerify bytes or the manifest binding.
  Rust packaging now decodes those proof bytes through BN254 G1/G2
  curve-membership checks, including G2 subgroup preflight, so off-curve or
  non-subgroup 12-word Groth16 tuples are rejected before Torii emits
  deployment-bound EVM or TRON contract-call payloads. JavaScript and Python
  portal helpers mirror the G1/G2 curve-equation and G2 subgroup checks before
  wrapping Groth16 prover results, emitting direct EVM/TRON wallet calldata, or
  forwarding lower-level Torii `proofBytesHex` / `proof_bytes_hex` query and
  submit fields; Swift, Kotlin, and Java Android raw bridge-submit clients
  apply the same checks before posting deployment-bound bridge DTOs.
  Torii artifact, proof-job, bridge-proof submit, and bridge-message submit
  paths now accept external `proof_bytes_hex` plus TRON
  `tron_verifier_address` deployment material, validated as a checksummed TRON
  Base58Check address by the relay clients, so relays can package the same
  deployment-bound proof bytes exposed by the SDK prover wrappers. Torii now
  fails EVM/TRON Groth16 artifact and proof-job packaging as a bad request when
  the deployment material is present but the external Groth16 proof tuple is
  missing, instead of falling through to generic signer/FastPQ package
  construction. Torii now
  rejects empty, all-zero, or non-384-byte external EVM/TRON Groth16 proof
  bytes before constructing a deployment-bound package, and the Rust,
  JavaScript/Python typed Torii clients, Swift SDK, Kotlin SDK, and Java
  Android SDK reject placeholder or non-canonical `proofBytesHex` plus
  malformed TRON verifier addresses before making artifact, proof-job,
  bridge-proof, or bridge-message requests. Torii
  typed artifact and proof-job clients also bind external EVM/TRON Groth16
  `proofBytesHex` / `proof_bytes_hex` to the normalized request message id and
  SORA source-domain word before network I/O, so a valid proof tuple for one
  SCCP message cannot be replayed into another artifact/job query.
  Torii
  typed submit clients now also use the local `message_bundle` to reject
  cross-source or replayed EVM/TRON Groth16 tuples before posting bridge-proof
  or bridge-message DTOs; Rust, Swift, Kotlin, and Java Android raw JSON bridge
  submit helpers enforce the same tuple/message-bundle binding. Torii
  now validates supplied EVM/TRON destination and proof fields before the
  disabled-lane readiness fallback, including canonical tuple roundtrip and
  tuple binding to the SCCP message id, SORA source-domain word, and commitment
  root, so malformed or cross-source relay material is not masked by a generic
  lane-not-ready response; strict disabled lanes still discard validated
  deployment bindings and proof bytes instead of exposing relay material while
  production readiness is false, but Torii retains the validated destination
  binding internally for configured rollout and all-lanes launch checks so the
  disabled-lane discard step cannot bypass rollout governance. Rust, Python,
  and JavaScript typed clients plus the bridge-feature CLI forward the same
  destination/proof query material with canonical 384-byte BN254 tuple,
  G1/G2 curve validation, and G2 subgroup validation before network I/O. Rust,
  JavaScript, and Python query and submit clients plus Swift/Kotlin/Java
  Android raw and typed submit clients now reject off-curve BN254 G1/G2 tuple
  coordinates and on-curve non-subgroup G2 points before network I/O, and
  the typed clients reject deployment destination fields when the required
  `proof_bytes_hex` is absent or a standalone `proof_bytes_hex` lacks
  deployment destination fields, so operators cannot fetch incomplete
  production EVM/TRON submission packages through any primary typed client or
  the bridge-feature CLI. The same Rust, bridge CLI, web, Python, and mobile
  SDK preflight now rejects partial deployment tuples:
  proof bytes must be paired with the full EVM field set
  (`network_id_hex`, `verifier_address_hex`, `bridge_address_hex`,
  `verifier_code_hash_hex`, `verifier_key_hash_hex`,
  `expected_destination_binding_hash_hex`) or the full TRON field set
  (`network_id_hex`, `tron_verifier_address`, `verifier_code_hash_hex`,
  `verifier_key_hash_hex`, `expected_destination_binding_hash_hex`), and mixed
  EVM/TRON destination material is rejected locally. Torii's direct
  destination-material parser now enforces the same all-or-nothing rule before
  destination binding construction or disabled-lane fallback. Rust, web,
  Python, and mobile bridge-proof submit clients also enforce Torii's bundle
  selection before network I/O: exactly one of `burn_bundle` or
  `message_bundle` must be supplied, and deployment destination proof material
  is valid only with `message_bundle`.
  Rust, Swift, Kotlin, and Java Android raw JSON bridge submit helpers now also
  reject empty, all-zero, or non-384-byte snake-case `proof_bytes_hex`, missing
  proof bytes when destination deployment fields are present, or proof bytes
  without destination deployment fields before posting deployment-bound DTOs;
  those raw-submit preflights also require
  `message_bundle.commitment.message_id` and `message_bundle.commitment_root`
  whenever proof bytes are submitted with a message bundle, bind the proof tuple
  to that bundle context, and shape-check recognized destination hashes, EVM
  addresses, network IDs, and TRON verifier addresses before network I/O.
  Python Torii typed artifact, proof-job, bridge-proof, and bridge-message
  clients additionally keep TRON deployment material exact by rejecting padded
  Base58Check verifier addresses and surrounding/internal whitespace in inline
  network id, verifier code/key hash, expected binding hash, and proof-byte
  fields before request serialization. JavaScript, Swift, Kotlin, and Java
  Android Torii submit/query preflights now apply the same exactness to
  string-based TRON verifier addresses and deployment/proof hex before request
  serialization while still accepting already-byte proof tuples. Swift,
  Kotlin, and Java Android EVM-family, TON, TRON, and Substrate-family mobile
  prover request builders also reject padded fixed-width
  payload/proof-context hashes before deriving proof transcripts. Their shared
  SCCP source-proof helpers apply the same exact hash rule to source-adapter
  deployment binding and source-proof transcript hashes. Kotlin and Java
  Android additionally reject non-canonical decimal finality heights at the
  text parser boundary, while Swift keeps finality heights typed as `UInt64`.
  JavaScript web portal TON proof requests and source-adapter deployment
  bindings now have matching regressions for padded fixed-width hashes and
  leading-zero finality heights before app-linked prover callbacks run.
  Swift/Kotlin/Java Android shared SCCP source-proof helpers reject padded TRON
  source-event and raw-header hex before transcript hashing.
  The Rust, JavaScript, Python, Swift, Kotlin, and Java Android clients now
  expose bridge-proof and bridge-message submit helpers for relays and mobile
  apps posting those deployment-bound DTOs; Swift, Kotlin/JVM, and Java Android
  now also provide typed bridge-proof submit request wrappers that encode into
  the same Torii preflight path used by raw JSON submissions.
  Rust/Torii packaging now rejects zero deployment network ids, zero statement
  hashes, zero required public-input fields, wrong target domains, and
  same-source/target domain public inputs before emitting EVM/TRON Groth16
  relay packages. Rust EVM/TRON Groth16 contract submission builders also
  require transparent `target_domain` to equal the manifest counterparty
  domain, so local-domain SORA public inputs cannot be packaged for
  counterparty contract calls. Their submission templates use the canonical
  `submitSccpMessageProof(bytes,bytes32[6],bytes32)` signature, pinning
  emitted EVM/TVM calldata to selector `0xbd57826c`. Counterparty submission
  package construction now also fails closed when the manifest's envelope
  encoding is unsupported or cannot be reconstructed, rather than emitting an
  empty or generic relay envelope. The Rust TRON destination-binding helper
  also refuses non-SORA source-domain ids, same-source/target manifests,
  manifests whose counterparty target domain is not TRON, and non-`stark-fri-v1`
  proof families.
  Rust, JavaScript, Python, Swift, Kotlin, and Java Android now expose the
  canonical BN254 public-signal derivation helper used by those EVM/TRON
  Groth16 circuits, including statement and destination-binding signal words.
  The Rust TRON package builder and proof verifier now parse the
  deployment-binding key and recompute the canonical TRON binding hash before
  accepting relay packages, so tampered binding hashes fail even if the
  submission arguments and envelope are rebuilt consistently.
  The JavaScript, Python, Swift, Kotlin, and Java Android SDK surfaces also
  expose typed EVM-family and TRON Groth16 proof-request/prover wrappers,
  binding the canonical public inputs, SCCP bundle bytes, source proof bytes,
  statement hash, destination binding hash, and fixed BN254 signal words before
  an app-linked Groth16 prover emits proof bytes. Those UI-prover request
  builders and Rust proof-result wrappers now fail closed on unsupported
  transparent-public-input versions, zero statement/destination hashes, zero
  required public inputs, zero Groth16 target domains, and same-source/target
  domains before any app-linked prover result is accepted, and TRON request
  builders also require the paired SORA -> TRON destination lane. Their TRON
  source-call calldata helpers are locked to the production TRON -> SORA source
  lane and reject zero
  source-event digests before UI/mobile prover transcript derivation. Those SDK
  source-proof helpers now also derive ETH Deneb/Fulu execution-payload,
  beacon-body branch, and beacon header SSZ roots from UI/mobile witness
  material, matching the source-adapter checks for `execution_payload_branch`
  and `beacon_finalized_root`. The JavaScript
  package entrypoint now re-exports those SCCP helpers at runtime, matching the
  TypeScript declarations. JavaScript, Python, Swift, Kotlin, and Java Android now also
  package EVM-family and TRON wrapped Groth16 proof results into
  `submitSccpMessageProof(bytes,bytes32[6],bytes32)` contract-call calldata
  with selector/envelope bytes, six transparent ABI public-input words, and
  proof-result binding checks that revalidate proof context, request hashes, and
  envelope hashes before portal and mobile wallet submission.
  JavaScript, Python, Swift, Kotlin, and Java Android
  SDKs now also expose `substrate-runtime-v1` proof-request/prover wrappers for
  SORA-Kusama, SORA-Polkadot, and SORA2 destination lanes, locked to SORA-origin
  source domains and binding the source domain, canonical transparent public
  inputs, length-prefixed SCCP bundle/source proof bytes, statement hash, and
  destination binding hash
  before an app-linked runtime prover emits proof bytes. A TRON/TVM Solidity
  deployment entrypoint now wraps
  the shared immutable BN254 verifier under `contracts/tron/sccp/`; its
  `submitSccpMessageProof(...)` path recomputes the self-addressed TRON
  destination binding from the actual deployed runtime code hash, governed key
  hash, and lane metadata, then records accepted message ids to block replay.
  The TRON constructor rejects missing or mismatched key hashes, empty
  proof-family labels, proof families other than `stark-fri-v1`, zero network
  ids, non-SORA source-domain ids, non-TRON target domains, and
  same-source/target domains, and its submission path rejects zero
  statement/public-input fields, wrong target-domain words, and Groth16 proof
  envelopes with non-canonical ABI length or whose version, message id,
  cleartext source-domain word, source-domain width, or commitment root does not
	  match the configured lane and public inputs before verifier dispatch. Accepted
	  EVM and TRON proof events now include the SCCP statement hash and destination
	  binding hash, and the EVM wrapper exposes `destinationBindingHash()`, so live
	  canary logs can be audited against the exact governed statement and deployed
	  binding. The shared contract smoke now pins its temporary `solc`, `ganache`,
	  and `ethers` dependencies and runs with quiet Ganache logging, keeping the
	  deterministic BN254 acceptance/replay check reproducible for operator
	  validation. The TRON source bridge constructor also
  rejects any non-SORA target domain before it can emit governed source-call
  configuration.
  The shared Solidity smoke now builds a deterministic self-consistent BN254
  test proof and submits it through both the EVM Groth16 wrapper and the
  TRON wrapper, covering positive pairing acceptance, accepted-event fields,
  public-input preflight failures, source-domain overflow, and replay rejection
  alongside malformed-proof rejection.
  Production rollout still requires deploying it and recording the deployed
  code/key hashes in governed destination binding material; the offline TRON
  evidence helper can now recompute that destination binding hash, compare it
  with an expected governed value, and operators can query the same value from
  the wrapper's `destinationBindingHash()` view or the post-deploy
  `DestinationBindingConfigured` canary event during rollout.
  The lane readiness surface now separates source material from deployment
  evidence; exact configured source material can set only the source-material
  readiness bit, while external consensus, receipt/message-inclusion, and
  trust-anchor readiness require the matching configured source-adapter engine
  deployment record. Source material by itself cannot mark the source adapter
  production-ready or satisfy the deployment-aware production proof helpers.
  TRON lane-level readiness has explicit regression coverage for the exact
  source deployment, destination rollout, and route allowlist combination, plus
  replayed source, destination, and route material failures.
  Deployment-aware source proofs now bind the configured source-adapter
  deployment hash and deployment receipt hash inside
  `SccpSourceVerifierEvidenceV1`, whose hash is part of the adapter
  OpenVerify statement. Material-only source proofs remain diagnostic artifacts
  and fail the configured production deployment path even when the lane's source
  verifier material otherwise matches. The configured admission verifier now
  splits diagnostic unready handling from production admission: it can tolerate
  an unready outbound destination manifest for deployment-governed lanes, but it
  still requires non-SORA source proofs to satisfy the production
  material-and-deployment gate.
  Bridge proof admission validates SORA-origin Nexus finality separately from
  non-SORA source-chain envelopes. Nexus block-level SCCP message records are
  restricted to
  SORA-origin payloads; external-source messages must enter through bridge proof
  submission with their source-chain envelope. Disabled SCCP lanes remain
  non-consumable in state-changing Torii endpoints and on-chain bridge proof
  admission even if historical unready-proof diagnostics are enabled in config.
  SORA-origin Nexus finality proofs now carry the full Sumeragi vote-signing
  material, including parent/post state roots, chain-order hash, re-chain
  sequence, and optional highest-QC reference. `iroha_sccp` exposes a
  BLS-normal aggregate verifier for those proofs, validates validator PoPs,
  and enforces the same quorum threshold as core finality verification before
  treating the proof as production-grade.
  Torii no longer synthesizes non-SORA source-chain envelopes from local Iroha
  finality; external-source submissions must carry source-adapter proof
  envelopes. Rust, JavaScript, Python, Swift, Kotlin, and Java SDKs now expose
  local-first Solana proof requests plus TON shard-state and TON
  full-light-client audit role proof requests so web and mobile UIs can collect
  source witness data, invoke an app-linked prover, and submit the resulting
  proof on-chain without relying on node-side proof generation. Rust now also
  wraps TON final proof bytes into the same request/envelope-hashed result
  object used for proof-result submission packaging. TON
  proof request builders are now locked to the TON source domain, and Solana
  source-proof witness/request builders are locked to the Solana -> SORA lane,
  preventing portal/mobile code from producing cross-domain local prover
  requests before request hashing or prover invocation. The Solana
  full-light-client audit helpers now share a cross-SDK golden vector for the
  Tower replay, full AccountsDB lattice, and bank/fork-choice roles, including
  statement hashes and FastPQ public-input columns, so web and mobile prover
  transcripts stay byte-identical to the verifier-facing canonical form.
  The TON user-side proof helpers and source adapter now bind full masterchain
  and basechain shard BlockIdExt context, including workchain ids, shard ids,
  seqnos, block hashes, and file hashes, and dictionary-backed
  `ShardStateUnsplit.accounts` openings must match the explicit basechain
  shard id and seqno supplied to the local prover request. The TON masterchain
  config-proof helpers and verifier also pin the active validator-set opening
  to config parameter `34` through a bounded TON `HashmapE 32 ^Cell`
  dictionary proof BoC, bind that 32-bit key width into the config-proof
  transcript, and decode the proven config-34 `ValidatorSet` cell into SCCP's
  canonical validator-set payload, so portal/mobile provers cannot treat an
  arbitrary config leaf, abstract branch, or independently supplied roster as
  the active validator set. The Rust, web, Python, Swift, Kotlin, and Java
  Android transcript builders now also reject config-proof and transition inputs
  with wrong versions/domains, zero masterchain/config/validator hashes,
  mismatched config-34 BoC payload/leaf/validator-set hashes, non-adjacent
  validator-set sequence numbers, or signature proofs signed over a different
  transition message. This removes the remaining zero-file-hash, generic-shard,
  generic-config-leaf, placeholder config-branch, config-roster, and
  transition-message transcript gaps in the current TON UI/mobile
  proof-generation surface. TON source-adapter admission now also requires the
  governed full-light-client audit bundle to be present as role-separated
  OpenVerify/FastPQ proof capsules for masterchain config, validator-set
  transition, and shard-accounts dictionary verifiers, so the remaining TON
  production blockers are governed live verifier deployments, canaries, route
  rollout, and destination rollout rather than app-side request binding.
  Readiness diagnostics now report that deployment-evidence blocker instead of
  the already-implemented shard-state proof evaluation path. The
  offline `scripts/sccp_all_lanes_evidence.py` preflight now merges rendered
  source, destination, and route TOML snippets and fails with lane-specific
  blockers unless every advertised SCCP remote domain has source material,
  source-adapter deployment evidence, destination rollout material, and route
  allowlist material before governance staging. The same preflight recomputes
  the audited Solana and TON full-light-client gate hashes plus the TRON source
  bridge config hash from governed fields and invokes each lane's canonical
	  source evidence validator, preventing non-zero placeholders, template-derived
	  component hashes, or non-canonical source-adapter verifier keys from
	  satisfying rollout review. The Rust source-material/deployment gates,
	  standalone source evidence renderers, and aggregate preflight now also
	  reject reused non-zero role digests across trust anchors, consensus
	  verifiers, message-inclusion verifiers, source-state verifiers, source bridge
	  code/config hashes, adapter VKs, deployment receipts, and audited Solana/TON
	  verifier roles before governance staging. Focused TON source-state evidence
	  tests now also pin rejection when a full light-client audit hash is replayed
	  from the source trust anchor, adapter verifier VK, or deployment receipt
	  hash. It also rejects lane-foreign
	  Solana or TON full-light-client audit fields, and SORA-bound audit fields
	  replayed on non-SORA target deployments, before governance staging, matching
	  the runtime deployment-shape gate and its core all-lanes admission regression
	  coverage before audit gate hashes are recomputed. Public release-bundle
	  verification now also requires each source-adapter `gate_hash` to equal the
	  lane's named final gate transcript rather than an arbitrary audit role hash,
	  so Solana tower replay, TON masterchain-config, or other component verifier
	  hashes cannot be promoted into public production evidence. Shared
	  source-adapter OpenVerify admission also
	  rejects all-zero proof bytes before decode, so placeholder adapter proof
	  envelopes cannot reach lane-specific verifier-key, schema, or public-input
  checks. It now also validates destination verifier
  identities with the lane-specific address/program/runtime parsers, preserves
  helper-emitted destination binding metadata comments, stores explicit
  destination binding fields in rollout config, and recomputes or compares the
  governed SORA -> ETH/BSC, Solana, TON, TRON, and Substrate-family destination
  binding hashes before accepting rollout records. EVM-family helpers now emit
  the canonical deployment binding key, and both the preflight and runtime
  readiness gates require that key to be present and match the deployment tuple.
  Solana, TON, TRON, and Substrate-family records must also carry the canonical
  binding key, and runtime readiness rejects native records that include EVM/TRON
  network or bridge-wrapper fields.
  TRON rollout records also fail if their explicit `destination_network_id`
  drifts from the governed source bridge network id used for the SORA -> TRON
  binding. The ZK consensus policy hash includes those destination binding
  fields so governed rollout evidence is committed by policy, not only by
  operator comments.
  Ready lanes report canonical source material, source-adapter deployment
  record hashes, destination binding summaries, and the recomputed
  route-allowlist evidence hash in the preflight JSON for governance
  comparison.
  JavaScript, Python, Swift, Kotlin, and Java Android now also expose the
  EVM-family and TRON Groth16 proof request wrappers for portal and mobile
  prover flows, with TRON wrappers locked to the SORA -> TRON lane, EVM-family
  wrappers locked to the governed SORA -> ETH/BSC destination lanes, and
  EVM-family, TON, and TRON wrappers rejecting empty SCCP bundle bytes before
  local request-hash derivation. TON/Substrate proof-result wrappers now reject
  all-zero external proof bytes before deriving request-bound envelope hashes;
  EVM-family and TRON wrappers additionally enforce the canonical 384-byte
  Groth16 ABI length, and JavaScript/Python portal surfaces plus
  Swift/Kotlin/Java Android mobile SDKs now parse that tuple before wrapping or
  submitting proofs so the version, embedded message id, source-domain width,
  commitment root, and BN254 coordinate ranges fail closed before wallet
  calldata is emitted. Those same tuple checks now bind the embedded message
  id and commitment root to the transparent public inputs and the embedded
  source domain to the wrapped/submitted request context. JavaScript, Python,
  Swift, Kotlin, and Java Android now
  package those wrapped EVM-family/TRON proof results into production verifier
  contract-call calldata and reject mismatched proof bytes, public inputs,
  statement hashes, destination-binding hashes, or public signal words before
  handing bytes to a wallet or relayer. The JavaScript package entrypoint now
  also exports the low-level transparent public-input ABI-word encoder and
  checked `submitSccpMessageProof(...)` calldata encoder for web portals that
  package wallet calls directly. The JavaScript, Python, Swift, Kotlin, and
  Java Android direct calldata encoders now apply the same SORA source-domain
  proof-tuple check before emitting wallet calldata, so portal and mobile
  callers cannot bypass the higher-level submission wrapper with a mismatched
  Groth16 source-domain word. TON request builders also
  require the exact mainnet shard-state light-client verifier id plus a non-zero
  source-state verifier hash before local prover invocation.
	  JavaScript and Python
	  local-prover facades now also isolate the request object passed into
	  app-linked prover callbacks. The JavaScript and Python Solana/TON
	  source-state prover facades snapshot caller-supplied OpenVerify/FastPQ
	  requests into frozen callback objects with defensive-copy byte getters
	  before proof bytes are wrapped. Kotlin/JVM and Java Android mobile
	  prover facades now also hand app-linked proof engines request snapshots,
	  including Solana AccountsLtHash and full-light OpenVerify/FastPQ
	  source-state callbacks, across TON, Solana, EVM-family, TRON, and
	  Substrate-family proof flows
	  while wrapping returned bytes against the original canonical request,
	  and JavaScript/Python source-state callback
	  result metadata
	  (`version`, proof family, circuit id, and exact canonical proof base64)
	  must match the active request and returned proof bytes. The
	  EVM-family/TRON/Substrate
	  facades reject explicit callback result metadata that does not match the
	  active request hash, envelope hash, backend, EVM-family/TRON transparent
	  public inputs, EVM-family/TRON proof context, EVM-family/TRON public signal
	  words, optional exact proof-base64 text, Solana proof-context hash, or
	  TON/Solana source-adapter deployment-binding hash. Python and JavaScript
	  now reject whitespace-padded proof-base64 aliases instead of trimming them,
	  preventing stale UI prover outputs or callback-side request mutations from
	  being repackaged under a different on-chain submission context.
	  JavaScript and Python Solana proof-result wrappers now also reject
	  object-shaped callback results whose optional source-proof public inputs,
	  proof context, source-state verifier id/hash, or source-adapter deployment
	  binding metadata disagrees with the canonical SDK-built request, so UI
	  prover metadata cannot be silently discarded and replaced before
	  submission packaging.
	  EVM-family/TRON optional callback metadata is
  strict when present, so `null`/`None` backend, request/envelope hash,
  public-input, proof-context, statement/destination-binding hash, or
  public-signal fields fail instead of collapsing to omitted metadata.
  JavaScript and Python EVM-family, TRON, and Substrate-family local-prover
  surfaces now also rebuild the canonical production request before invoking
  app-linked callbacks and before deriving proof-result envelope hashes, so web
  portals and portal backends cannot wrap proof bytes around manually mutated
  request hashes, public signal words, proof contexts, lane backends, or target
  domains.
  JavaScript, Python, Swift, Kotlin, and Java Android EVM-family/TRON
  submission builders also require wrapped `proofBase64` to match wrapped
  `proofBytes` before contract-call calldata is emitted, matching the existing
  Solana proof-result integrity guard. Those wrapped EVM-family/TRON proof
  results now carry the original request bundle/source-proof bytes, and
  proof-result based submission builders rebuild the canonical request hash
  before emitting calldata, so stale UI/mobile proof results cannot be replayed
  against a swapped SCCP bundle. Substrate-family proof results expose the same
  request bytes for runtime-proof chaining, and the JavaScript TypeScript
  declarations plus Python package `__all__` exports now publish those
  proof-result request-byte fields and wrapper helpers to portal/mobile
  integrators. The JavaScript TypeScript declarations also expose named
  local-prover callback result types for Solana, TON, EVM-family, TRON, and
  Substrate-family facades so portal UIs can return the request/envelope hash,
  backend, binding-hash, proof-context, public-input, and public-signal
  metadata that the runtime already validates. TON TypeScript declarations now
  keep pre-proof request construction separate from post-proof message-body
  submission packaging, so `buildTonSccpProofRequest`/`TonSccpProver` no longer
  advertise proof bytes, wrapped proof results, manifest metadata, or query ids
  as prover input fields. The Python package root now exports every public SCCP
  helper/class/constant from `iroha_torii_client.sccp`, including Solana
  submission entrypoint metadata and TON audit-role verifier ids used by portal
  proof backends, and its package-root regression now derives that full public
  surface from the module so future proof helpers cannot be added only behind a
  deep import. The JavaScript package entrypoint now exports the same portal
	  constants at runtime and in TypeScript declarations, including the fixed
	  transparent public-input byte length, Solana submit entrypoint, and TON
	  full-light-client audit verifier ids. It also re-exports the Solana
	  full-light audit request builders, source-state capsule canonicalizers,
	  finality/vote transcript helpers, and account-inclusion tree helpers from
		  the package root so TypeScript portal imports match the packaged runtime
		  surface. The package export regression now also compares every runtime
		  SCCP export against `index.d.ts`, so portal TypeScript declarations stay
		  aligned with newly exported proof helpers such as the BSC commit-message
		  and commit-seal transcript builders.
	  JavaScript TON requests and results
	  now also freeze the callback-visible envelope and nested context/binding
  objects, expose request/proof byte fields through defensive-copy getters, and
  Python local-prover requests, callback inputs, proof results, and Solana
  submissions now use dict/list-compatible read-only envelopes so portal
  backends cannot mutate derived request hashes, proof contexts, or submission
  arguments after canonicalization.
  Swift, Kotlin, and Java Android TON wallet/liteserver submissions now expose
  the same version, `internal_message` kind, verifier entrypoint, argument
  vector, and envelope bytes/hex as the web/Python SDKs while retaining
  defensive BOC/envelope byte getters; the same mobile SDKs can build the TON
  message-body submission input directly from a local `TonSccpProofResult`, so
  apps no longer need to manually copy proof-context hashes between proof
  generation and wallet/liteserver packaging. JavaScript Substrate-family
  runtime proof requests/results now use the same frozen envelope and defensive
  byte getter contract, and the JavaScript package root now re-exports the
  Substrate runtime proof backend id, request builder, and prover facade so
  TypeScript portal imports match the packaged runtime surface. It now also
  exposes Substrate-family `scale_call_v1` runtime-call
  submission builders, and the Python, Swift, Kotlin, and Java Android SDKs
  mirror the same `SccpBridge.submit_message_proof` argument order so
  portal/mobile-generated proofs can be handed directly to chain submission
  clients without node-side proof generation.
  The package root also re-exports the SCCP source-adapter OpenVerify circuit id, FastPQ
  parameter-set id, and verifier VK hash helper used by portal evidence
  checks, keeping declared TypeScript imports runtime-available.
  Swift, Kotlin, and Java Android proof-result wrappers now rederive the
  canonical request before hashing the proof envelope, Java Android EVM-family,
  Solana, TON, and TRON proof/submission results return defensive byte copies,
  and Kotlin EVM-family, Solana, TON, TRON, and Substrate-family
  request/result/submission objects now also return fresh copies for request
  byte fields and proof bytes, closing the mobile path where a manually
  constructed or mutated request object could otherwise supply stale envelope
  context. Kotlin Solana AccountsLtHash and TON shard-state source-state proof
  capsules now also defensively copy prover-returned proof bytes before those
  bytes are hashed into full-light-client audit requests, and Java Android now
  mirrors the TON proof-family/circuit-id null guards before hashing those
  capsules. JavaScript, Python, Swift, Kotlin, and Java Android TON
  local-prover calls now preflight the canonical production request before
  invoking the app-linked proof engine and reapply that guard when wrapping
  proof bytes, matching the Solana SDK guard pattern across web portal,
  backend, and mobile UI proof generation. Swift EVM-family, TRON, TON, and
  Substrate production wrappers now preserve omitted optional source-proof
  bytes through proof wrapping and submission packaging while still rejecting
  non-empty all-zero source-proof placeholders. Deployment-aware SCCP
  production source-proof extraction now enters through the deployment-aware
  bundle-structure gate, so configured material and source-adapter deployment
  evidence are checked consistently before accepting a source-chain proof
  envelope. Torii's app API artifact, proof-job, runtime SCALE export,
  bridge-proof submit, and bridge-message submit paths now resolve that same
  configured source lane from ZK config before wrapping UI-generated
  source-chain proof envelopes, so production Solana/TON/TRON/EVM-family proofs
  are submitted on-chain against governed source-adapter material instead of the
  static placeholder manifest.
  TON wallet/liteserver message-body builders apply the same non-empty,
  non-all-zero proof-byte and empty-bundle rejection when callers package a
  submission directly, now require TON-targeted transparent public inputs, and
  enforce the bounded 4096-cell TON message-body BOC cap before wallet or
  liteserver payloads are emitted. They also recheck wrapped TON proof results
  against the mainnet shard-state verifier profile plus canonical TON -> SORA
  source-adapter deployment binding before accepting request-bound envelope
  hashes. Wrapped TON proof results now carry the original request
  bundle/source-proof bytes, and proof-result based submission builders rebuild
  the canonical request hash before producing wallet/liteserver payloads.
  JavaScript, Python, Swift/iOS, Kotlin/JVM, and Java Android now reject
  standalone TON proof-byte payloads at submission packaging time, so UI/mobile
  apps cannot submit proof bytes against a swapped SCCP bundle after local
  proof generation.
  EVM-family and
  TRON proof-result submissions now apply the same bundle/source-proof request
  hash reconstruction before contract calldata is emitted. EVM-family, TON, and
  TRON request hashes also length-prefix bundle and source-proof bytes so the
  transcript binds their boundary. JavaScript and Python TON submission
  metadata canonicalizers now also reject versionless or lane-foreign manifests,
  non-`stark-fri-v1` proof families, non-`ton-contract-v1` verifier backends,
  TON public-input domain drift, and destination-binding overrides that differ
  from the manifest before portal/backend BOC packaging. The JavaScript and
  Python BOC builders now pass the root `destinationBindingHash` into that
  metadata canonicalizer and reject any manifest/metadata binding mismatch, and
  Swift, Kotlin/JVM, and Java Android expose matching typed mobile metadata
  canonicalizers for wallet packaging. The JavaScript TypeScript manifest
  declaration now exposes the required V1 field and pinned TON proof/backend
  labels to portal callers. Swift, Kotlin/JVM, and
  Java Android TON proof requests plus direct wallet/liteserver message-body
  builders now also reject all-zero statement and destination-binding hashes,
  matching the web/Python portal guard before mobile proof engines or wallets
  see placeholder submission context.
  The TRON/TVM contract bundle also includes `SccpTronSourceBridge`, an
  owner-governed source emitter for the production
  `submitSccpSourceEvent(uint32,uint32,bytes32)` transaction-call proof. It
  stores lane-specific immutable metadata, rejects mismatched source/target
  domain arguments, rejects zero or replayed source-event digests, and emits the
  canonical indexed `SccpSourceEvent(bytes32)` log shape used by legacy receipt
  diagnostics while the production adapter proves the successful call under
  java-tron's transaction Merkle root. That proof is
  pinned to java-tron's full serialized `Transaction` Merkle leaf hash rather
  than the public raw-data txID, and the Rust/SDK transaction-source helpers
  recompute the java-tron Merkle root from the supplied full transaction bytes,
  index/count, and branch before hashing the source transcript. The Rust
  transcript helper now also invokes the same successful source-call verifier as
  admission, while the JavaScript, Python, Swift, Kotlin, and Java Android SDK
  helpers preflight the serialized `Transaction` protobuf shape, success
  result, signature count/length, non-zero owner/contract addresses, and
  source-call calldata before deriving production transcript hashes. TRON
  header/witness signatures accept java-tron's raw recovery-id encoding while
  retaining low-S malleability checks. Production rollout still needs the live
  deployment address, runtime bytecode hash, live TOML metadata for the queried
  `sourceBridgeConfigHash()`, deployment receipt hash, and governed source
  material/deployment evidence recorded before the lane can be activated.
  The source bridge constructor now matches that production lane shape by
  requiring SORA's target domain id `0` for TRON -> SORA while rejecting any
  non-TRON source domain, any non-SORA target-domain id, and
  same-source/target deployment. Rust and Python source-bridge config-hash
  helpers enforce the same TRON -> SORA shape before deriving rollout evidence.
  JavaScript, Python, Swift, Kotlin, and Java
  Android source-call calldata helpers mirror that lane shape and reject any
  non-TRON source, non-SORA target, or zero source-event digest before
  generating `submitSccpSourceEvent(uint32,uint32,bytes32)` calldata.
  Python now mirrors the Solana local request, proof-context hash, wrapped proof
  result, and `borsh_instruction_v1` submission helper, and it builds the same
  deployment-bound TON request/result envelope for portal/operator backends.
  Python now also exposes the same canonical ETH/BSC receipt-proof, BSC
  validator-set payload, BSC ValidatorSet storage-value, metadata-proof, and
  transition-message, TON shard-proof, TON validator-set transition, TRON
  receipt-proof, and Substrate-family storage-proof transcript hash helpers as
  the web and mobile SDKs, so backend portal tooling can derive adapter-bound
  source proof hashes from collected source-chain witness material instead of
  accepting opaque placeholders.
  Source proof branch witnesses are now centrally bounded to 64 H256 siblings,
  matching the `u64` leaf-index depth used by the verifier, and over-depth or
  malformed branches fail before transcript hashing or root reconstruction.
  The Solana
  source adapter now cryptographically binds `message_proof_hash` to the source
  event digest, transaction-status root, raw 64-byte transaction signature, raw
  32-byte emitter program id, and a non-empty inclusion branch. The
  transaction-status Merkle leaf is derived from the source-event digest plus
  transaction identity, and source-chain inclusion proofs must carry that
  Solana-specific leaf before reconstructing the claimed transaction-status
  root with the SCCP `sccp:source:node:v1` Blake2b node hash. The SDKs expose
  the same helper, reject zero source-event digests,
  transaction-status roots, all-zero decoded transaction signatures, all-zero
  decoded emitter program ids, root/branch mismatches, or empty inclusion
  branches, and decode the UI-provided Solana base58 signature/program id
  before hashing so UI provers do not pass opaque placeholder message proof
  hashes. The JavaScript and Python helpers also reject duplicate camelCase and
  snake_case aliases for the Solana source-event digest, transaction-status
  root, transaction signature, emitter program id, and inclusion branch before
  deriving the message-proof hash or transaction-status branch root. Their
  active-stake and stake-history helpers reject duplicate aliases for validator
  public keys, validator stakes, activation epochs, and deactivation epochs
  before deriving epoch-stake-root, stake-activation, and stake-history
  transcripts.
  The Solana source adapter also verifies an embedded
  stake-weighted Ed25519 finalized-slot vote certificate: it recomputes the
  vote-message hash from the slot/header/status/message-proof material plus a
  shape-checked Solana finality-context hash, checks the unique non-zero
  validator roster hash against configured source trust-anchor material, caps
  the roster at 8,192 entries before expensive proof work, enforces strict
  `> 2/3` signed stake, and rejects malformed context, tampered signatures, or
  replayed vote hashes. The Solana source-material profile now also binds
  mainnet-beta's 432,000-slot epoch length plus the generic SCCP
  source-event leaf/node Merkle prefixes, and signed finality contexts are
  rejected unless
  `epoch == finalized_slot / 432000` and `parent_slot + 1 == finalized_slot`.
  The adapter also requires `epoch_stake_root` to derive from the signed epoch
  plus active vote roster under `sccp:solana:epoch-stake-root:v1`. It now also
  requires
  `tower_lockout_hash` to derive from the signed epoch, finalized/rooted/parent
  slots, parent bank hash, and the 32-slot lockout depth under
  `sccp:solana:tower-lockout:v1`, and the JavaScript, Python, Swift, Kotlin,
  and Java SDKs expose the matching UI/mobile helpers. The adapter now also
  requires `tower_replay_hash` to derive from the signed epoch, rooted slot,
  finalized slot, direct parent slot, and explicit 31-vote active post-root
  Tower stack under
  `sccp:solana:tower-replay:v1`; the same JavaScript, Python, Swift, Kotlin,
  and Java SDK helpers/tests expose that UI/mobile transcript. The rooted slot
  supplies the 32nd Tower confirmation. The adapter now also requires
  `stake_activation_hash` to derive from the signed epoch, active
  vote roster, activation epochs, and deactivation epochs under
  `sccp:solana:stake-activation:v1`, and rejects validators that are not
  activated before that epoch. It also requires `stake_account_state_hash` to derive from the
  stake-activation hash, authorized voter keys, delegated stakes,
  activation/deactivation epochs, vote account addresses, stake account
  addresses, vote account state hashes, and stake account state hashes under
  `sccp:solana:stake-account-state:v1`, with matching JavaScript, Python, Swift,
  Kotlin, and Java helper tests. Those account state hashes must now derive from
  `sccp:solana:account-opening:v1` account-opening metadata that binds the
  account address, expected Vote/Stake owner program id, lamports, rent epoch,
  executable flag, and account-data hash; vote and stake openings owned by the
  wrong Solana program id or marked executable fail closed. The SDKs expose the
  matching account-opening hash helper for UI/mobile proof generation. The
  adapter now also binds vote-account opening data hashes to semantic
  vote-account transcripts under `sccp:solana:vote-account-data:v1` and
  stake-account opening data hashes to semantic stake-account transcripts under
  `sccp:solana:stake-account-data:v1`, with matching SDK helpers/tests. The
  SDKs can also parse raw Solana `VoteStateVersions::V1_14_11`/`V3`/`V4`
  account data into the vote-account transcript for UI/mobile proof
  generation, while the verifier now requires each raw vote-account buffer in
  the finalized vote proof to parse back to the same semantic transcript. Those
  Rust, JavaScript, Python, Swift, Kotlin, and Java Android parsers now reject
  malformed active Tower stacks before transcript hashing: confirmation counts
  must descend exactly, vote slots must remain strictly increasing after the
  rooted slot, the root cannot overlap the active post-root stack, and every
  raw authorized-voter map key, including future scheduled rotations, must be
  non-zero. Focused
  regressions cover parser and source-adapter admission for bad confirmation
  counts, repeated vote slots, and roots that collide with the first active
  vote. V4 vote accounts are now capped to the four-entry authorized-voter
  epoch window used by the current Anza vote-interface V4 max-size fixture
  before transcript hashing, while legacy V1/V3 layouts retain the 32-entry
  prior-voter ring validation.
  The parsers also consume the VoteState
  suffix: V1/V3 prior-voter cursor data
  must have a valid circular-buffer index and boolean empty flag, zero
  prior-voter pubkeys must carry zero epoch bounds, and non-zero prior-voter
  pubkeys must carry increasing epoch bounds. Epoch-credit history is capped to
  Solana's 64-entry bound, must be sorted/monotonic, and must not include
  epochs after the signed finalized-bank epoch; the last-timestamp tuple must
  either be the default `(0, 0)` or stay at-or-before the newest parsed Tower
  vote slot with a non-negative timestamp, and remaining fixed account padding
  must be zero.
  Legacy V1/V3 vote accounts derive Solana's V4 default collector and
  commission fields from the vote account address and node pubkey; V4 account
  buffers bind the collector pubkeys, basis-point commission fields, pending
  delegator rewards, and optional compressed BLS pubkey directly, with raw V4
  commission fields capped to 10,000 bps and present V4 BLS keys required to be
  non-zero across Rust, JavaScript, Python, Swift, Kotlin, and Java Android
  before transcript hashing. The
  finalized vote proof also carries each raw 200-byte
  `StakeStateV2::Stake` account buffer, and the verifier requires the parsed
  raw stake account to match the bound semantic stake-account transcript,
  including the known Solana 8-byte legacy/current warmup-cooldown-rate slot
  and the `StakeFlags` byte; reserved stake-flag bits and unsupported
  warmup/cooldown encodings now fail closed across Rust and the web/mobile SDK
  parsers, with Java Android mirroring Kotlin's supported Solana `0.25`/`0.09`
  byte policy. The
  adapter now also binds the signed finality context to the fixed
  `SysvarStakeHistory1111111111111111111111111` account opening owned by
  `Sysvar1111111111111111111111111111111111111`; that opening's data hash must
  derive from Solana's bincode vector sysvar account-data layout under
  `sccp:solana:stake-history-sysvar-data:v1`: a little-endian `u64` entry count
  followed by newest-first `(epoch, effective, activating, deactivating)` `u64`
  records. SDK helpers still accept sorted ascending witness entries for replay
  and reverse them only for the sysvar account-data hash; SDK raw-data helpers
  and the verifier-side vote proof now also validate and hash the exact raw
  StakeHistory sysvar bytes.
  The
  adapter now also requires
  `stake_history_hash` to derive from the signed epoch, effective voting stakes,
  delegated stake-account stakes, activation/deactivation epochs, the
  stake-account state hash, and a sorted StakeHistory sysvar window containing
  the signed epoch under `sccp:solana:stake-history:v1`. It replays the
  Tower-era 900 bps warmup/cooldown schedule over that bounded window with
  integer arithmetic, requires each submitted effective stake to match the
  replayed validator status, requires the signed-epoch StakeHistory effective
  total to equal the replayed active validator roster, and exposes matching
  JavaScript, Python, Swift, Kotlin, and Java helper tests. The adapter now also
  requires deterministic
  SCCP account-inclusion branches for every vote account opening, stake account
  opening, and the StakeHistory sysvar opening, with vote and stake account
  addresses disjoint across both roles. The verifier hashes exact raw
  account/sysvar data, folds account-inclusion leaves and branch siblings into
  `account_inclusion_root`, and requires that root to be bound into the signed
  finality context. SDK account-inclusion root helpers reject zero leaf hashes
  and cap sibling branches at 64 nodes to match Rust source-adapter admission.
  SDK opened vote-account and stake-account vectors are also capped at 8,192
  entries per role before account-inclusion or AccountsLtHash proof material is
  derived, matching the source-adapter validator bound.
  The same JavaScript, Python, Swift, Kotlin, and Java SDKs
  expose account-raw-data, account-inclusion leaf/node/root, and branch-builder
  helpers for portal and mobile proof generation. Their raw StakeHistory sysvar
  hash helpers also require the bincode vector records to be in Solana's
  canonical newest-first order, matching the Rust verifier-side canonical sysvar
  bytes before UI/mobile proof flows derive the sysvar-data hash. The adapter now also requires
  `bank_fork_hash` to derive from the signed epoch, finalized slot,
  direct parent slot, bank signature count, parent bank hash, finalized bank
  hash, blockhash, transaction-status root, account-inclusion root,
  AccountsLtHash checksum, and optional hard-fork hash data under
  `sccp:solana:bank-fork:v1`. The verifier now recomputes Agave's
  SHA-256 bank internal-state hash from parent bank hash, signature count,
  blockhash, raw AccountsLtHash, and optional hard-fork data, requiring it to
  equal the adapter bank hash. Full-bank AccountsLtHash witnesses are also
  rejected when they are the neutral all-zero vector at the verifier and
  JavaScript/Python/Swift/Kotlin/Java SDK request boundaries. Raw zero
  checksum helpers remain representable for diagnostics, but opened-subset
  proof transcripts now reject an all-zero residual so the vote/stake/sysvar
  rows cannot claim to exhaust the finalized bank lattice. The
  finality context also binds
  `accounts_lt_hash_proof_public_inputs_hash`, derived from the canonical
  `sccp:solana:accounts-lt-proof-public-inputs:v1` recursive proof
  public-input transcript covering the source domain, backend id, genesis hash,
  epoch, finalized/direct-parent slots, bank signature count, bank hashes,
  blockhash, transaction-status root, account-inclusion root,
  AccountsLtHash checksum, optional hard-fork data, and derived bank-fork hash,
  with matching JavaScript, Python, Swift, Kotlin, and Java helper tests. The
  full AccountsDB lattice audit statement now binds the completed nested
  `accounts_lt_hash_proof` capsule hash directly rather than substituting only
  the public-input transcript hash, so second-stage audit proofs are tied to
  the actual user-generated source-state proof bytes. SDK
  proof-request witnesses now canonicalize Solana blockhashes to `0x` 32-byte
  hex and hash the raw blockhash bytes, so base58/hex UI inputs bind to the
  same source proof transcript. JavaScript and Python AccountLtHash helpers now
  also require the account-opening `executable` flag to be a real boolean, so
  UI/backend strings such as `"false"` cannot silently alter Agave-compatible
  lattice rows before source-state proof requests are built. Production Solana SDK prover wrappers also
  reject missing, empty, or oversized transaction-status inclusion branches
  before invoking linked provers or wrapping externally generated proof bytes,
  matching the source adapter's non-empty, 64-sibling branch requirement. They
  also require the request and witness `mainnetGenesisHash` to equal Solana
  mainnet-beta's canonical genesis hash before packaging production proof
  bytes, and the source-state wrapper overloads now fail closed if the
  originating OpenVerify/FastPQ public-input columns no longer bind the Solana
  source domain and mainnet-genesis column. They reject the Rust template
  AccountsDB source-state verifier hash, and require the full 2,048-byte
  nonzero AccountsLtHash witness so portal/mobile proof flows cannot package
  checksum-only bank-state material. The lower-level
  AccountsLtHash public-input transcript helpers now also replay the supplied
  full AccountsLtHash against both the BLAKE3 checksum and Agave bank hash
  before returning bytes/hashes in Rust, JavaScript, Python, Swift, Kotlin, and
  Java Android, so direct helper calls and full-light audit statement builders
  fail closed on checksum-only or stale-bank-hash material.
  Solana source verifier material and source adapter deployment records now
  also carry the mainnet AccountsDB recursive verifier identity plus deployed
  `source_state_verifier_hash`, so production readiness cannot be declared with
  only a generic finalized-slot/status verifier profile. Matching Solana source
  material plus deployment metadata is still structurally verifiable but no
  longer opens production by itself while the full light-client verifier stack
  remains outstanding. Production Solana
  adapter proofs now also carry a nested `accounts_lt_hash_proof`
  `SccpSourceStateVerificationProofV1` OpenVerify/FastPQ capsule with circuit id
  `sccp-solana-accounts-lt-hash-v1`; the verifier checks that capsule against the
  deployed `source_state_verifier_hash`, finalized-bank public-input schema, and
  FastPQ proof before accepting source material that is otherwise
  production-ready, with fail-closed coverage for wrong circuit ids, backend
  tags, schema descriptors, auxiliary envelope data, public-input columns, and
  backend proof bytes; oversized source-state OpenVerify capsules are rejected
  before decode. The OpenVerify schema descriptor now embeds the governed
  AccountsDB source-state verifier id and verifier hash, matching the FastPQ
  context and SDK proof-request builders, so UI/mobile provers cannot receive a
  source-state request whose schema is detached from the deployed verifier
  material. The capsule now also binds
  `opened_accounts_lt_hash_contributions_hash`, a canonical transcript of the
  opened vote/stake/sysvar account rows and their Agave account `AccountLtHash`
  contributions, so a user-generated proof cannot detach the recursive
  AccountsLtHash boundary from the account subset checked by the adapter witness.
  It also binds `opened_accounts_lt_hash_residual_checksum`, derived by
  subtracting the opened-subset aggregate from the supplied full-bank
  `accounts_lt_hash` under Agave's wrapping `u16` lattice arithmetic, so the
  nested proof records the algebraic residual that must be covered by the future
  full AccountsDB lattice proof. That residual must be nonzero before the
  transcript is hashable. JavaScript, Python, Swift, Kotlin, and Java
  Android SDKs now derive this opened contribution transcript from account
  openings/raw data for web portal and mobile provers, while still accepting
  precomputed 2048-byte Agave `AccountLtHash` rows when a proof engine supplies
  them directly. Those supplied rows are no longer trusted as opaque proof
  material: JavaScript, Python, Swift, Kotlin, and Java Android now recompute
  each opened vote, stake, and StakeHistory sysvar `AccountLtHash` from the
  paired opening/raw-data preimage and reject stale or mismatched rows before
  request hashing or local prover invocation. Python and Swift now also have
  pure BLAKE3 checksum and XOF fallbacks for AccountsLtHash request
  validation/derivation, matching the
  Kotlin/Java mobile path without requiring optional native BLAKE3/Norito
  bindings, and max-size raw account data parity vectors now cover the
  many-chunk BLAKE3 tree/XOF path across Python/Swift/Kotlin/Java.
  The verifier also rejects duplicate opened account addresses
  across the vote-account, stake-account, and StakeHistory sysvar roles, and
  the JavaScript, Python, Swift, Kotlin, and Java Android transcript builders
  reject the same duplicate-address witness before hashing or local proof
  packaging. Those opened-role paths also reject zero-lamport vote, stake, and
  StakeHistory sysvar openings even though generic Solana AccountsLtHash
  arithmetic keeps zero-lamport accounts as the neutral contribution, so a
  witness cannot alias one AccountsDB address into multiple roles or hide a
  live Solana account role behind an identity LtHash row. It also recomputes the deterministic account-inclusion tree for
  exactly those opened leaves and rejects branches rooted in a larger tree with
  extra unopened leaves. JavaScript, Python, Swift, Kotlin, and Java Android now
  expose an exact opened-account inclusion witness helper so web portal and
  mobile proof code can derive the verifier-side root and split branches from
  the same opened account inputs instead of hand-assembling tree leaves; those
  helpers now reject duplicate opened account addresses across vote, stake, and
  StakeHistory sysvar roles before deriving branch vectors. The JavaScript web
  and Python SDK account-opening and account-inclusion leaf helpers also reject
  duplicate aliases for account address, owner program id, rent epoch,
  account-data hash, finalized slot, opening object, raw data, raw-data hash,
  and nested opening address fields, and they recompute `rawDataHash` from raw
  account data when both forms are supplied. Their opened contribution, opened
	  inclusion witness, and Agave bank-hash helpers now extend that guard to
	  opened vote/stake arrays, StakeHistory sysvar fields, account-inclusion roots,
	  AccountsLtHash checksum/root fields, full AccountsLtHash bytes, parent bank
	  hashes, bank signature counts, blockhash bytes, and optional hard-fork hash
	  data before deriving residual, branch, or bank-state transcripts. The
	  lower-level Tower lockout/replay, bank-fork, and AccountsLtHash public-input
	  helpers now reject duplicate finalized-slot, epoch, rooted/parent slot,
	  parent-bank hash, bank hash, bank-fork hash, Tower vote-slot,
	  transaction-status root, account-inclusion root, AccountsLtHash
	  checksum/root, full AccountsLtHash, and hard-fork data aliases before
	  hashing. The JavaScript web
	  SDK also freezes the returned account-inclusion tree and opened-account
  witness objects plus their branch arrays and declares them readonly for
  TypeScript portal consumers, preventing post-derivation mutation before local
  proof callbacks consume the witness. The
  Tower replay transcript now also carries the derived bank-fork hash, and all
  SDK helpers require that hash before deriving `sccp:solana:tower-replay:v1`,
  so a rooted-vote stack cannot be replayed against a different finalized
  bank-state statement. All Solana full-light audit roles now expose
  `mainnet_genesis_hash` plus the common `epoch`, `rooted_slot`, `parent_slot`,
  `vote_message_hash`, and `accounts_lt_hash_proof_hash` OpenVerify
  schema/public-input columns across Rust, JavaScript, Python, Swift, Kotlin,
  and Java Android, so portal/mobile provers submit the Solana chain identity,
  finality window, voted message commitment, and nested AccountsLtHash proof
  commitment directly. The bank/fork-choice full-light
  audit role also exposes `account_inclusion_root`, `bank_signature_count`,
  `bank_hash_hard_fork_data_hash`, and `tower_replay_hash` as explicit columns,
  so portal/mobile provers submit the opened-account root, Agave signature
  count, optional hard-fork data hash, and Tower replay root as
  verifier-visible inputs rather than only through the aggregate audit statement
  hash. The Tower replay full-light audit role now also exposes
  `stake_account_state_hash`, `stake_history_sysvar_account_hash`, and
  `account_inclusion_root` as explicit OpenVerify schema/public-input columns,
  so user-side provers submit the opened vote/stake account-state commitment,
  StakeHistory sysvar account commitment, and account-inclusion root directly
  with the Tower lockout/replay hashes.
  This
  is an
  incremental cryptographic source-engine slice, not yet a complete Solana
  light client; full Tower BFT vote-account/state replay beyond the bound
  31-vote active post-root stack plus rooted confirmation transcript,
  replacing the current reference AccountsLtHash OpenVerify/FastPQ capsule with a
  deployed full AccountsDB lattice verifier, and full bank-state/fork-choice rule
  evaluation
  remain open. The same
  JavaScript, Python, Swift, Kotlin, and Java SDK
  surfaces now expose canonical ETH/BSC receipt-proof, BSC validator-set
  payload, BSC ValidatorSet storage-value, metadata-proof, and
  transition-message, ETH sync-committee transition payload, TON shard-proof,
  TON masterchain block-message/signature, TON validator-set transition payload,
  TRON receipt-proof, TRON transaction-source proof, TRON
  witness-schedule transition payload, and Substrate-family storage-proof plus
  authority-set transition-message/justification transcript helpers, so web,
  operator, and mobile tooling can derive every adapter-bound source proof hash
  from collected witness material before invoking the linked prover. The ETH/BSC
  source adapters now derive
  `receipt_trie_proof_hash` from the source event digest, receipt root,
  finality witness, receipt-trie index, bounded MPT proof nodes, and inclusion
  branch instead of accepting any non-zero receipt proof placeholder. They also
  open the receipt trie under the finalized ETH execution receipts root or BSC
  `receipts_root`. Non-placeholder ETH/BSC material now requires the proven
  value to decode as an actual successful legacy or typed EVM receipt whose log
  topics match the canonical SCCP source event ABI
  (`keccak256("SccpSourceEvent(bytes32)")`, `source_event_digest`) with empty
  event data and whose log emitter equals the governed source bridge emitter
  address carried by production source material and source-adapter deployment
  evidence; the parser rejects failed
  receipts, non-minimal cumulative gas, malformed logs, logs with more than
  four topics, non-32-byte topics, digest-only matches, bad bloom lengths,
  wrong emitters, and invalid typed-prefix byte `0x00`, while allowing
  unrelated valid `LOG0` entries that do not satisfy the SCCP source-event ABI.
  Placeholder
  structural fixtures may still use the typed EVM-family receipt-root MPT
  envelope carrying the SCCP receipt/message root. The ETH source adapter now
  also verifies an embedded beacon sync-committee certificate by deriving the
  ordered BLS committee trust-anchor hash, checking proof-of-possession values,
  recomputing the signed sync-committee message hash, verifying the aggregate BLS
  signature, and enforcing strict `> 2/3` signed committee weight. ETH adapter
  proofs now also carry raw execution-header RLP; the verifier Keccak-hashes it
  to the claimed execution block hash, parses the RLP header fields, and checks
  the block-number and receipts-root fields against the SCCP finality height and
  adapter execution receipts root. The BSC source adapter also verifies an
  embedded secp256k1 validator-set commit-seal certificate by deriving the
  validator-set trust-anchor hash from validator addresses and powers,
  recovering signed validators from 65-byte seals over the BSC commit-message
  hash, binding the seal hash into the adapter transcript, and enforcing strict
  `> 2/3` signed power. BSC active receipt proofs now enforce the Parlia mainnet 200-block
  epoch window, and validator-set transitions must advance exactly one epoch on
  that epoch's start block. The Rust and JavaScript, Python, Swift, Kotlin, and
  Java Android transition-message helpers reject non-BSC source domains,
  non-adjacent validator epochs, and transition blocks that are not exactly
  `to_validator_epoch * 200` before deriving a signed Parlia transcript. BSC
  validator-set transition proofs now carry a
  canonical next-set payload, hash it under
  `sccp:bsc:validator-set-payload:v1`, decode the address/power list, and
  require the decoded payload to derive the advertised next validator-set hash.
  BSC transitions now also prove mainnet ValidatorSet storage parity by opening
  the `0x0000000000000000000000000000000000001000` account under the transition
  header state root, verifying its storage root, and opening
  `currentValidatorSet.length` plus each carried validator
  `currentValidatorSet[index].consensusAddress` slot before the next set can
  activate. The offline
  `scripts/sccp_bsc_source_bridge_evidence.py` helper now renders the governed
  BSC -> SORA source material and source-adapter deployment TOML from live
  validator-set, verifier, source bridge, adapter verifier, and deployment
  receipt hashes, while rejecting non-production lanes and zero evidence. BSC
  still needs recursive verifier deployment before it is a complete
  light-client engine.
  ETH now derives the Deneb/Fulu SSZ `ExecutionPayloadHeader` root from the
  execution RLP header, opens the fixed beacon-body execution-payload branch,
  and recomputes the signed `BeaconBlockHeader` root before accepting the
  finalized root. The offline `scripts/sccp_eth_source_bridge_evidence.py`
  helper now renders the governed ETH -> SORA source material and
  source-adapter deployment TOML from live beacon, verifier, source bridge,
  adapter verifier, and deployment receipt hashes, while rejecting
  non-production lanes and zero evidence. ETH still needs recursive verifier
  deployment and any production light-client update/state branches not
  discharged inside that deployed source-adapter circuit. The ETH/BSC,
  Solana, TON, TRON, and
  Substrate-family source-material gates now accept only exact verifier profiles
  binding the planned backend, finality policy, and inclusion-proof layout, and
  the component hashes must be deployment-supplied rather than the built-in
  template hashes; generic source material remains rejected. Destination
  rollout readiness is now likewise profile-bound for all advertised domains:
  generic anchor metadata,
  cross-chain verifier identities, malformed addresses, and zero verifier
  addresses fail closed, and TON destination rollout helpers now pin raw
  contract identities to the exact mainnet anchor id. The EVM destination
  evidence helper now renders exact ETH/BSC destination rollout and route
  allowlist records while recomputing the wrapper-bound destination binding
  hash, closing the hand-assembled EVM destination rollout tooling gap. Route
  allowlist readiness is now profile-bound as well: every advertised counterparty requires the exact
  governed route allowlist id plus a non-zero policy hash, while missing,
  generic, malformed, or cross-domain allowlist material remains rejected.
  A combined lane-readiness helper now evaluates source verifier material,
  source-adapter deployment evidence, destination rollout material, and route
  allowlist material together, so operator tooling can prove an individual
  lane's local production readiness while still rejecting cross-domain replay
  of any component. Source-adapter OpenVerify verification now fails closed when
  the lane-specific verifier-key commitment cannot be reconstructed, and
  regression coverage rejects wrong verifier-key hashes, backend tags, schema
  descriptors, auxiliary envelope data, public-input columns, and backend
  proof bytes across the typed source-adapter variants.
  Solana submission
  packages now also carry the statement hash, destination binding hash, and
  proof context hash alongside proof bytes, public inputs, and bundle bytes, so
  verifier-program submissions are
  replay-scoped to the manifest deployment context. Solana SDK proof requests
  require the same statement hash and destination binding hash as a proof
  context, and wrapped UI-prover results commit to that proof-context hash as
  well as the source witness hash. JavaScript, Python, and Swift now expose
  explicit proof-result wrappers for externally generated UI prover bytes
  across Solana, TON, EVM-family, TRON, and Substrate-family lanes, while
  Kotlin and Java Android expose the same flow through public
  `wrapProofResult` helpers. Those direct wrappers bind proof bytes to the
  canonical request before deriving request-bound envelope hashes; Solana also
  rebuilds the canonical request from witness/context before accepting
  externally generated proof bytes. Those SDK proof request surfaces now also
  carry the configured source-adapter deployment hash, deployment receipt hash,
  and canonical deployment-binding hash in prover public inputs and wrapped
  proof results, while leaving the Solana verifier-program
  `proof_context_hash` tied only to statement and destination binding for
  submission compatibility. JavaScript, Python, Swift, Kotlin, and Java Android
  Solana submission builders now require both explicit transparent SCCP message
  public inputs and a wrapped SDK `proofResult` before wallet/RPC packaging;
  wrapped `proofResult.publicInputs` are source-proof inputs and are not
  accepted as a substitute for transparent submission inputs. The JavaScript
  distributable and TypeScript declarations now require that wrapped result as
  well, so browser portal code cannot compile against the raw proof-byte-only
  path. JavaScript, Python, Swift, Kotlin, and Java Android Solana source-state
  proof capsule wrappers now also require a full SDK-built AccountsLtHash or full-light audit
  OpenVerify/FastPQ request shape before wrapping externally generated proof
  bytes, including the Solana source domain, canonical FastPQ parameter set,
  deployed AccountsDB verifier id/hash, AccountsLtHash direct-parent and
  residual hashes, full-light audit role metadata, and the matching
  OpenVerify public-input columns. The TypeScript declaration
  requires the full request union instead of a minimal circuit-id object.
  Swift, Kotlin, and Java Android typed wrapper overloads now reject
  hand-built request values unless the SDK-built statement/context/schema
  bytes, public-input columns, FastPQ public inputs, and transitions are
  present before source-state proof bytes are wrapped. Those
  builders
  also require the wrapped result backend, proof-context hash, non-zero envelope
  hash, deployment-binding hash, source-state verifier id/hash, submitted proof
  bytes, and source-proof statement/destination binding to match the submission
  context. The JavaScript, Python, Swift, Kotlin, and Java
  Android Solana prover facades now refuse to invoke app-linked provers or wrap
  proof bytes unless that request is bound to the production AccountsDB
  source-state verifier id plus non-zero source-state verifier, deployment, and
  deployment-receipt hashes; diagnostic zero-binding request builders remain
  fixture-only. Those Solana SDK surfaces now also build the
  nested AccountsLtHash source-state proof request for UI/mobile provers,
  including statement bytes, opened-account commitment bytes, verification
  context, OpenVerify schema descriptor, mainnet-genesis public-input binding,
  public-input columns, and FastPQ
  transition payloads. The JavaScript, Python, Swift, Kotlin, and Java
  Android SDKs now also build the matching `borsh_instruction_v1` Solana
  program-instruction envelope from UI-generated proof bytes, canonical
  transparent public inputs, SCCP bundle bytes, statement hash, destination
  binding hash, and proof-context hash, rejecting context-hash mismatches before
  wallet/RPC submission. JavaScript and Python also reject caller-supplied
  Solana `publicInputsBytes` when they do not equal the canonical transparent
  public inputs, while the mobile SDKs derive those bytes internally. The
  JavaScript and Python Solana submission builders now also reject explicit
  `null`/`None` values for `publicInputs`, `proofBytes`, `proofContext`,
  `statementHash`, and `proofContextHash` instead of treating them as omitted
  and falling back to wrapped proof-result metadata. Those submission builders
  now require `publicInputs.targetDomain = Solana`, matching
  the Rust SORA -> Solana verifier-program template, and also reject any
  `destinationBindingHash` other than the canonical SORA -> Solana binding
  hash. JavaScript
  now freezes the Solana local-prover
  request/result/submission objects and returns proof/instruction byte fields
  through defensive-copy getters, preventing browser code from mutating request
  hashes or packaged on-chain bytes after the app-linked prover runs. The
  callback-visible Solana witness snapshot also deep-freezes nested UI payload
  metadata and copies nested byte buffers before invoking the prover, so portal
  state cannot be mutated through the proof callback; the
  TypeScript declarations mark those Solana SDK objects as readonly for portal
  compile-time checks. Kotlin mobile now passes app-linked Solana proof engines
  a byte-array snapshot of the canonical request and wraps returned proof bytes
  against the original request hash, so callback-side array mutation cannot
  corrupt the submitted envelope binding. Python mirrors those Solana
  request/result/submission immutability guarantees with read-only
  dict/list-compatible envelopes for
  backend portal tooling. The JavaScript and Python Torii clients now preserve
  those Solana binding and context fields when decoding typed SCCP artifact/job
  responses, so the web portal and operator tooling can audit the exact
  deployment replay scope before preparing the wallet transaction. TON submission
  packages now carry a real `ton_message_body_boc_v1` message body BOC, with
  proof bytes, public inputs, SCCP bundle bytes, destination binding, and
  statement hash bound into the TON internal-message payload; the Python SDK now
  exposes the same BOC and read-only submission-envelope builders as the web and
  mobile SDKs for portal/backend packaging. JavaScript and Python TON
  submission builders plus the Swift, Kotlin, and Java Android TON message-body
  constructors now also accept wrapped request-bound proof results directly,
  rechecking proof bytes, transparent public inputs, request hash,
  source-adapter deployment-binding hash, envelope hash, statement hash,
  destination binding, and proof context before constructing the BOC payload;
  the dynamic JavaScript/Python path and the Swift/Kotlin/Java mobile typed
  inputs now require that wrapped proof result instead of accepting standalone
  raw proof bytes.
  The TON source
  adapter now derives `shard_proof_hash` from the source event digest,
  masterchain seqno/block hash, shard block hash, shard state root,
  transaction root, and inclusion branch, so the masterchain/shard witness can
  no longer be any non-zero placeholder hash. The signed TON masterchain
  block-message transcript now also binds the masterchain `BlockIdExt`
  workchain id `-1`, masterchain shard `0x8000000000000000`, root hash, and a
  non-zero file hash across Rust plus JavaScript, Python, Swift, Kotlin, and
  Java Android helpers, so a basechain or file-hash-free block id cannot be
  signed as masterchain finality. TON SDK proof requests now also
  bind the SCCP statement hash, destination binding hash, source-state verifier
  id/hash, source-adapter deployment hash, deployment receipt hash, and
  canonical deployment-binding hash into the user-side request/envelope hash,
  length-prefix bundle/source-proof bytes before request hashing, and now reject TON proof
  requests whose backend is not `ton-contract-v1` or whose source-adapter
  deployment binding is zero/zero, so mobile, web, and Python portal provers do
  not generate deployment-agnostic or non-contract TON proof bytes. That
  deployment binding is now fixed to the governed TON -> SORA source lane across
  JavaScript, Python, Swift, Kotlin, and Java Android, and JS/Python reject
  nested binding inputs that try to supply a non-SORA target domain. The
  Python Torii client now also builds the deployment-bound TON local proof
  request/result envelope and decodes the production `ton_message_body_boc_v1`
  platform payload with `message_body_boc`, query id, destination binding, proof
  bytes, public-input bytes, bundle bytes, and statement hash, matching the
  JavaScript portal surface. The TON source adapter now also verifies an
  embedded masterchain validator-signature certificate by deriving the ordered
  Ed25519 validator-set trust-anchor hash, recomputing the signed masterchain
  block-message hash, checking the signature-capsule hash, verifying Ed25519
  validator signatures, and enforcing strict `> 2/3` signed validator weight.
  TON validator-set transition proofs now derive the active validator set from a
  configured parent trust anchor by binding the parent set, canonical next
  validator-set payload hash, payload-derived next set, next-set config hash,
  masterchain block, and seqno range, then requiring a strict `> 2/3`
  parent-set Ed25519 signature capsule. The transition chain now also requires
  adjacent validator-set seqnos and strictly increasing masterchain transition
  seqnos, preventing skipped TON validator updates and out-of-order transition
  replay. TON validator-set payloads/signature proofs are capped at 1024
  validators, reject all-zero Ed25519 validator keys, and ordered
  validator-set transition chains plus shard-state/config source Merkle
  branches are capped at 64 entries before source-adapter evidence hashing. TON
  signature-proof transcript builders now also require signer bitmap padding,
  signature count, claimed total/signed weights, and the strict `> 2/3`
  signed-weight threshold to agree before serialization, and
  transition-signature builders reject parent validator-set hash or
  transition-message hash mismatches before proof submission.
  The TRON source adapter now treats bounded transaction source proofs as the
  production path: it hashes `transaction_bytes`, verifies the java-tron
  transaction Merkle branch against the signed-header `txTrieRoot`/adapter
  `transaction_root`, parses one successful `TriggerSmartContract` transaction
  with exactly one canonical recoverable secp256k1 signature over
  `sha256(raw_data)` from the configured owner, and requires calldata
  `keccak256("submitSccpSourceEvent(uint32,uint32,bytes32)")[0..4] ||
  abi_word_u32(source_domain) || abi_word_u32(target_domain) ||
  source_event_digest` to the governed source bridge contract. The adapter derives
  `receipt_proof_hash` from the transaction-source transcript binding the source
  digest, receipt root, transaction root, transaction index/count, transaction
  bytes, transaction Merkle branch, and source inclusion branch only after
  recomputing the java-tron transaction Merkle root from those transaction bytes
  and branch. The Rust public transcript helper now fails closed unless the
  same transaction bytes satisfy the successful governed source-call verifier,
  and JavaScript, Python, Swift, Kotlin, and Java Android helper surfaces reject
  malformed or non-source-call TRON transactions before UI/mobile prover
  transcript hashing. Bounded MPT
  `receipt_trie_proof_nodes` remain a legacy structural transcript only, where a
  proven `TransactionInfo` value may carry exactly one successful result field
  plus the SCCP source-event ABI topic, `source_event_digest`, and empty event
  data; unknown fields inside each parsed legacy log fail closed. Placeholder
  fixtures may still use the bounded typed RLP
  `sccp:tron:receipt-root-value:v1` envelope; legacy exact 32-byte roots are
  rejected. The DPoS receipt
  proof witness can no longer be any non-zero
  placeholder hash. The shared MPT verifier now handles canonical inline child
  nodes by traversing the raw embedded RLP node and rejects duplicate unused
  inline proof entries. JavaScript, Python, Swift, Kotlin, and Java Android SDK
  helpers now derive the same receipt-state MPT transcript and typed
  receipt-root MPT value envelope so portal and mobile provers do not need to
  hand-roll those encodings. The same TRON receipt-proof and receipt-state
  helpers reject zero source-event digests, receipt roots, and transaction roots
  before deriving transcript hashes; the typed TRON receipt-root MPT value
  helpers reject zero receipt roots as well. Rust and SDK TRON receipt-proof,
  receipt-state, and transaction-source transcript helpers also require a
  non-empty SCCP source inclusion branch before hashing, matching source
  envelope admission. Rust TRON solid-block message
  transcripts also reject wrong source domains, zero block heights, and zero
  block/schedule/receipt/transaction/proof hashes before deriving the signed
  message, while witness-seal and witness-schedule-transition seal hashes now
  require internally valid signed certificates, strict `> 2/3` witness weight,
  message binding, and next-schedule payload/hash consistency before hashing.
  It also verifies a
  stake/weight-style TRON witness seal: the adapter derives the witness schedule
  hash from 21-byte TRON addresses and weights, binds it to the solid-block
  message hash, recovers secp256k1 signers to TRON addresses, checks the
  configured witness-schedule trust anchor, and enforces strict `> 2/3` signed
  witness weight. TRON header and witness recoverable secp256k1 signatures now
  accept java-tron's raw recovery ids plus normalized `27..=30` fixture ids,
  reject invalid `r` scalars, high-S malleable encodings, and out-of-range
  recovery ids before proof acceptance, and require solid-block header proof
  hashes to recover both child and parent signatures to their declared TRON
  witnesses. Witness schedule
  rosters are capped at 64 unique addresses in
  the verifier and SDK payload helpers, and canonical schedule payload builders
  reject non-zero per-witness weights whose sum cannot fit the `u64`
  `totalWeight` committed by witness seals. TRON solid-block header proofs now
  parse the raw `BlockHeader.raw_data` bytes, verify the SHA-256 raw-data hash
  and block-number-derived TRON block id, allow only java-tron's known
  solid-header fields plus optional `witness_id`, bind `txTrieRoot` to the
  adapter `transaction_root`, prove the immediate signed parent header link, and
  recover child, parent, and bounded ancestor block producers' secp256k1 header
  signatures to active 21-byte TRON witness addresses. The ancestor chain is
  capped at 64 signed headers, linked by parent block ids, and required to step
  backward by one height with strictly decreasing timestamps. TRON solid-block
  confirmation headers are also capped at 64 signed descendants, linked forward
  from the solid block id, and non-placeholder material requires unique active
  witness producers in that confirmation chain to carry more than two thirds of
  the active witness schedule weight before the block is treated as solid.
  JavaScript, Python, Swift, Kotlin, and Java Android source-proof helpers now
  reject zero or out-of-range `r`, high-S, zero-S, and out-of-range recovery-id
  child/parent header signatures before local TRON solid-block header proof
  hashing, matching the Rust verifier's first-pass signature canonicalization.
  The same Rust and SDK
  transcript helpers reject all-zero `0x41`-prefixed TRON witness addresses in
  raw block headers, solid-block header proofs, and witness-schedule payloads
  before deriving hashes, so zero witness placeholders cannot satisfy local or
  on-chain preflight.
  TRON witness-schedule transition proofs now derive the active
  schedule from a configured parent trust anchor by binding the parent schedule,
  canonical next witness-schedule payload hash, payload-derived next schedule,
  transition block, and schedule-epoch range, then requiring a strict `> 2/3`
  parent-schedule secp256k1 seal. Multi-step TRON transition chains must be
  bounded to at most 64 hops, epoch-contiguous, strictly increasing by
  transition block number, and anchored to the supplied solid, parent, or signed
  ancestor header evidence. The Rust transition-message and transition-seal
  helpers now reject non-TRON domains, skipped schedule epochs, zero transition
  blocks, zero transcript hashes, and stale transition-message hashes before
  signature verification. TRON source-adapter material verification and the
  TRON DPoS verifier helper now preflight bounded adapter shape before
  transcript/evidence hashing, so oversized or mixed legacy branches, MPT nodes,
  wrong adapter domains, zero block/root/seal/proof hashes, empty witness
  rosters, non-canonical signer bitmaps, mismatched witness weights/signature
  counts, insufficient signed witness weight, all-zero TRON witness addresses,
  truncated or non-canonical header/witness signatures, stale
  transition-domain/message/seal metadata, transition chains, or transition
  payloads fail before canonical adapter bytes are serialized.
  The generic TRON source-adapter binding path now also recomputes the witness
  schedule hash, solid-block message hash, and witness seal hash, so swapped
  schedule/seal transcripts fail before recursive verifier material is
  evaluated.
  Substrate-family adapters now
  derive `storage_proof_hash` from the source event digest, finalized block
  number, GRANDPA set id, authority set hash, events root, source-event leaf
  index, canonical `frame_system::Events` storage key, and inclusion branch
  instead of accepting a placeholder storage proof hash. They also verify an
  embedded GRANDPA authority certificate by deriving the ordered Ed25519
  authority-set trust-anchor hash, recomputing the finalized precommit-message
  hash, checking the justification hash, verifying Ed25519 signatures, and
  enforcing strict `> 2/3` signed authority weight. Substrate authority-set
  transition proofs now derive the active authority set from a configured parent
  trust anchor by binding the parent set, canonical next authority-set payload
  hash, payload-derived next set, transition block, and GRANDPA set-id range,
  then requiring a strict `> 2/3` parent-set GRANDPA justification.
  BSC, Solana, TRON, and Substrate-family production source-verifier material
  templates now bind
  those validator/vote/witness certificate transcript prefixes as well as their
  inclusion-proof and TRON receipt-state prefixes, closing the inclusion-only
  deployment-hash gap for those lanes. BSC source proofs also support an
  ordered validator-set
  transition chain from a configured parent trust anchor into the active
  validator set, with strictly increasing transition block numbers, raw
  transition header RLP checked against the transition block hash, and Parlia
  `extraData` extraction required to match the signed next validator-set payload
  plus the proven ValidatorSet account/storage metadata proof available through
  web, Python, Swift, Kotlin, and Java Android SDK proof-generation helpers, ETH
  source proofs now support an ordered sync-committee
  transition chain from a configured parent committee into the active committee,
  with the canonical next sync-committee payload available through the web,
  Python, Swift, Kotlin, and Java Android SDK proof-generation helpers,
  TRON source proofs now support an ordered witness-schedule transition chain
  from a configured parent schedule into the active schedule with the next
  witness-schedule payload available through the web, Python, Swift, Kotlin,
  and Java Android SDK proof-generation helpers and a 64-hop verifier cap, and
  TON source proofs now support ordered payload-derived validator-set
  transition chains with the next validator-set payload available through the
  same web, Python, Swift, Kotlin, and Java Android SDK proof-generation
  helpers, a 1024-validator payload/signature cap, zero validator-key
  rejection, signer bitmap/weight preflight, parent/message hash binding, and a
  64-hop transition-chain cap plus a 64-entry cap on shard-state/config source
  Merkle branches. TON masterchain config proof transcripts now bind the active
  validator-set payload hash, TON config parameter `34`, the opened config
  value hash, a bounded TON `HashmapE 32 ^Cell` dictionary proof BoC, and the
  decoded `validators#11`/`validators_ext#12` config-34 payload into the signed
  block-message hash; legacy abstract config inclusion branches must be empty.
  The signed TON masterchain
  block-message now carries the mainchain `BlockIdExt`
  workchain/shard/root/file-hash tuple, SDKs expose the
  signed TON masterchain block-message and
  validator-signature transcript helpers, TON shard-proof transcripts now bind
  and verify a shard-state opening from the message root into the signed shard
  state root, the Rust verifier plus JavaScript, Python, Swift, Kotlin, and
  Java Android SDKs can derive bounded complete TON BoC root hashes for UI proof
  material including CRC32C-checked BoCs, strict partial-byte cell padding
  checks, and pruned-branch/Merkle proof/Merkle update exotic cell hash-depth
  semantics including legacy maskless pruned-branch proof cells emitted by
  existing TON tooling, the same Rust verifier plus JavaScript, Python, Swift,
  Kotlin, and Java Android SDKs can now derive both generic `HashmapE n ^Cell`
  value-cell hashes and selected `ShardAccount.last_trans_hash` /
  `last_trans_lt` identities from bounded TON dictionary proof BoCs while
  failing closed on pruned selected paths and non-256-bit `ShardAccounts`
  account keys, and the TON source
  adapter can bind an optional shard-state
  `ShardStateUnsplit` proof BoC plus ShardAccounts root/key/proof BoC opening
  into the shard-proof transcript, extract the `accounts:^ShardAccounts`
  reference hash, validate the embedded `ShardIdent` constructor and
  `shard_pfx_bits <= 60` bound, require TON mainnet `global_id = -239`,
  require TON basechain `workchain_id = 0`, require the selected 256-bit
  account key to match the proven shard prefix, decode shard-state `seq_no`,
  `gen_utime`, `gen_lt`, and `min_ref_mc_seqno`, reject zero
  sequence/generation/logical-time placeholders, reject MasterChain-only
  `custom:(Maybe ^McStateExtra)` refs on basechain shard states, require
  `min_ref_mc_seqno` not to exceed the signed masterchain seqno, require the
  legacy shard-state branch to be empty in dictionary mode, and verify the selected
  `ShardAccount.last_trans_hash` and `last_trans_lt` before accepting the
  signed shard-state root; the same JavaScript, Python,
  Swift, Kotlin, and Java Android SDKs now expose local `ShardStateUnsplit`
  proof-root, accounts-root, and selected-account helpers for UI prover preflight
  and reject dictionary-backed shard-proof inputs whose shard-state branch is
  non-empty, whose selected account key is not a 256-bit TON account id, whose
  ShardIdent shape is malformed, whose shard-state proof is not from TON
  mainnet `global_id = -239`, whose proven `ShardIdent` is not from TON
  basechain `workchain_id = 0`, whose selected account key prefix does not
  match the proven shard prefix, whose shard-state `seq_no`, `gen_utime`, or
  `gen_lt` is zero, whose basechain `ShardStateUnsplit` carries a
  MasterChain-only `custom` ref, whose `min_ref_mc_seqno` is ahead of the signed
  masterchain seqno, or whose shard-state root, accounts root, or selected
  account last transaction hash/logical time do not match the
  submitted transcript fields, and TON source proofs now carry a
  `shard_state_verification_proof` OpenVerify/FastPQ source-state capsule that
  is mandatory when deployed TON source-state verifier material is configured,
  with JavaScript, Python, Swift, Kotlin, and Java Android TON full-light audit
  request builders deriving `shard_state_verification_proof_hash` from that
  capsule instead of accepting a hash-only stand-in,
  binding the masterchain/shard/config/selected-account proof inputs to the
  advertised verifier hash before the adapter proof is accepted, with
  fail-closed coverage for wrong OpenVerify circuit ids, backend tags, schema
  descriptors, auxiliary data, public-input columns, backend proof bytes, and
  oversized source-adapter OpenVerify envelopes or source-state proof labels
  before decode;
  deployment-backed TON source-adapter readiness can now open when exact
  non-placeholder source verifier material, matching source-state verifier
  deployment fields, adapter verifier commitment, and deployment receipt hashes
  are present;
  Substrate-family source proofs now support an ordered authority-set transition
  chain from a configured parent set into the active set, with the next
  authority-set payload and transition transcript hashes now available through
  the web, Python, Swift, Kotlin, and Java Android SDK proof-generation helpers.
  Torii's SCCP message proof, runtime SCALE envelope, proof artifact, proof job,
  and recent-message read paths now recover non-SORA bundles from verified
  on-chain bridge proof records, enforcing typed artifact backend/manifest
  binding, stored proof-range/finality-height agreement, and current production
  source-lane proof validation before serving the user-submitted source proof.
  The all-lanes preflight and public release-bundle verification now also reject
  source-adapter gate audit hashes that replay source material, source-adapter
  deployment, destination binding, route allowlist, route canary evidence, or
  sibling audit hash roles, and required source-gate blockers are promoted into
  the lane-level preflight blockers.
- Keep live-network signing inputs runtime-only and continue using generated
  per-validator deployment bundles rather than hand-edited production configs.

**Next checkpoints:** continue replacing remaining SCCP source-chain verifier
placeholders behind the typed adapter variants so ETH/BSC/Solana/TON/TRON/
Substrate consensus/finality and receipt/message inclusion are checked against
external chain rules. For BSC, use the new offline source-bridge evidence
renderer, which now rejects BSC EVM-family template component hashes before
rendering governance TOML and also rejects non-canonical BSC source-adapter
OpenVerify VK hashes, plus the EVM live source and destination evidence
collectors to query deployed source emitter, bridge, and verifier views, verify
runtime code/key hashes, require the canonical RPC chain id, governed bridge
`networkId()`, reject verifier/bridge address aliasing, and require audited
source-emitter and bridge-wrapper code-hash pins before live TOML rendering,
and render all-lanes-ready rollout TOML, then finish
recursive verifier deployment
evidence for the current Parlia-header plus ValidatorSet-storage transition
chain. The EVM live/source-live helpers now reject padded CLI chain ids,
component hashes, and JSON-RPC quantity/hex results before rendering receipt,
runtime-bytecode, source, or destination metadata. The EVM live helper's
rendered TOML now preserves the observed RPC chain
id and bridge wrapper runtime code hash as metadata comments, and the all-lanes
preflight rejects ETH/BSC source material and destination rollout records that
lack that live bytecode evidence. For ETH,
use the new offline source-bridge evidence renderer, which now rejects Ethereum
EVM-family template component hashes before rendering governance TOML and also
rejects non-canonical ETH source-adapter OpenVerify VK hashes, and the EVM live
source/destination evidence collectors to collect governed source-emitter,
wrapper, and verifier hashes with the canonical RPC chain id, governed bridge
`networkId()`, and audited source-emitter/bridge-wrapper code-hash pins, then
finish recursive verifier deployment and cover any remaining production
light-client update/state branches not discharged inside that deployed source-adapter
circuit. The ETH/BSC adapter proof shapes are now preflight-bounded across Rust
and the web/mobile SDK helper surfaces: ETH caps sync committees at 512
authorities and 64 transition proofs, while BSC caps validator sets at 255
validators and 64 transition proofs before transcript hashing. For
Substrate-family lanes, use the offline
source evidence renderer, which now rejects Substrate-family template component
hashes, including the runtime storage-proof verifier hash, and non-canonical
source-adapter OpenVerify VK hashes before rendering governance TOML using the
same runtime-storage template preimage as Rust. JavaScript, Python, Swift,
Kotlin, and Java Android runtime-storage request builders also reject that
template source-state verifier hash before invoking the app-linked prover, and
they derive the exact statement bytes, verification context, schema descriptor,
public-input columns, FastPQ public inputs, and metadata transitions from the
UI-collected `System.Events` storage proof witness. JavaScript and Python also
canonicalize the storage proof's source domain before merging flat or nested
source verifier material, while still rejecting nested material with duplicate
or mismatched source-domain aliases, so portal inputs can use the same flat
witness shape without bypassing material-domain checks. Use
the destination evidence renderer for governed material, runtime rollout, and
allowlist material, then extend the current GRANDPA
authority-certificate and authority-set transition checks, which are now
preflight-bounded to 2,048 authorities and 64 transition proofs and reject
all-zero authority keys across Rust and the web/mobile SDK proof helpers. The
Substrate-family production source-adapter gate now requires matching runtime
storage-proof verifier material, matching source-adapter deployment evidence,
and a submitted `SccpSourceStateVerificationProofV1` OpenVerify/FastPQ capsule
whose circuit id, schema descriptor, public inputs, verifying-key hash, and
FastPQ proof verify against the governed runtime-storage verifier hash. The
all-lanes summary now derives the matching `substrate_runtime_storage_gate_hash`
for each Substrate-family lane and the release-bundle verifier requires it in
the source-adapter audit hash set, so ready bundles cannot publish Substrate
source material without the runtime-storage proof gate. The Substrate
source-evidence renderer now mirrors that requirement before production TOML:
the expected source material, source-adapter deployment, and runtime-storage
gate hashes must all be supplied and match before `toml_ready` can become true.
The Rust admission path no longer has a metadata-only fail-closed sentinel for
Substrate-family lanes; the remaining blocker is governed live runtime verifier
deployment evidence plus lane rollout across the all-lanes launch policy.
For
Solana, use the offline
`scripts/sccp_solana_source_state_evidence.py` source-state renderer and the
destination evidence renderer to collect governed deployment hashes, then
extend the current stake-weighted Ed25519 vote-certificate and finality-context
binding, whose adapter shape is now preflight-bounded before transcript hashing
for validator vectors, fixed-width vote/stake account raw-data witnesses,
AccountsLtHash proof capsules, signer bitmaps, Tower vote stacks, and hard-fork
data, into a full
mainnet-beta light-client verifier covering Tower BFT
vote-account/state replay beyond the bound 31-vote active post-root stack plus
rooted confirmation transcript, proving that the supplied bank AccountsLtHash
checksum is canonical for the full finalized-bank AccountsDB lattice hash,
replacing the current reference AccountsLtHash OpenVerify/FastPQ capsule with
the deployed full AccountsDB lattice verifier, and full Solana
bank-state/fork-choice rule evaluation beyond the bound Agave internal-state
hash. The source-state renderer now rejects padded component hashes and
source/target domain values before deriving source material, source-adapter
deployment records, or full-light-client gate hashes.
For TON, use the offline source-state and destination evidence renderers to
wire the live deployed full light-client/verifier contract evidence into
governance for the default catalog and keep the deployment-backed source
adapter gate covered by live receipt/material fixtures. The TON live
destination collector now treats `0x` values as exact 32-byte hex and otherwise
requires canonical strict base64/base64url for verifier code, account-state, and
last-transaction hashes, so malformed or non-canonical TON API hash text cannot
be normalized into rollout evidence. It also requires the returned `accountStates` account
address to be canonical and equal to the requested verifier contract before
pinning live hashes, rejects API URLs that carry credentials, params, queries,
or fragments, caps the `accountStates` JSON body and HTTP error details before
decoding, and requires runtime API keys to be exact ASCII tokens without
whitespace or control characters. It also rejects duplicate keys in the remote
JSON object graph instead of accepting last-value-wins parsing. The source-state
verifier id/hash request fields are now mirrored through the JavaScript,
Python, Swift, Kotlin, and Java Android portal/mobile SDK surfaces, and those
builders require the mainnet verifier id plus a non-zero verifier hash before
the UI/mobile prover runs. They now also reject the Rust/evidence-renderer
template-derived TON shard-state verifier hash in both generic source-material
audit helpers and TON shard-state request builders, so app-side proof
generation cannot promote profile-template material before on-chain
submission. JavaScript, Python, Swift, Kotlin, and Java Android
now also expose canonical source-adapter verifier-key commitment helpers for
all ETH/BSC/Solana/TON/TRON/Substrate-family source lanes, so web portals and
mobile apps can display or audit the exact `adapter_verifier_vk_hash` accepted
by Rust admission and the offline governance evidence renderers. The Solana
  AccountsLtHash source-state request
  builders now expose the exact OpenVerify/FastPQ statement, schema,
  mainnet-genesis public-input binding, public-input columns, and transition
  payloads to web/mobile proof engines while rejecting
  the Rust template AccountsDB source-state verifier hash. The Solana
  Rust AccountsLtHash proof builder now applies the same production-ready
  source-state verifier predicate before packaging an OpenVerify/FastPQ capsule,
  so template-derived AccountsDB verifier hashes cannot be used even through
  direct helper calls. The Solana
  source-material template now commits to those AccountsLtHash OpenVerify/FastPQ
  constants plus the SCCP source-event leaf/node and Solana transaction-status
  leaf Merkle prefixes. SDK Solana transaction-status branch helpers now fold
  UI-collected inclusion siblings with that same SCCP source-node Blake2b hash,
  and Solana proof-result/submission builders reject all-zero proof bytes, so
  template drift or placeholder local prover output is caught before governed
  evidence rendering or on-chain submission. The same JavaScript, Python,
  Swift, Kotlin, and Java Android submission handoff also rejects wrapped proof
  results whose `publicInputs.sourceStateVerifierId` or
  `publicInputs.sourceStateVerifierHash` diverge from the top-level audited
  source verifier fields, so tampered UI/mobile proof-result metadata cannot
  present a different source-state verifier before wallet submission. They also
  pin wrapped proof-result, proof-context, source-adapter deployment-binding,
  and Solana transparent-public-input versions to `v1`, require `proofBase64`
  to match the proof bytes, require non-zero witness and proof-context hashes,
  and reject non-adjacent finalized/parent slots, zero bank signature counts, or
  zero bank/source-state hash fields in the wrapped source-proof public inputs
  before wallet submission. Rust
  counterparty submission
  package builders and transparent-proof structure verification now enforce the
  same non-empty, non-all-zero proof-byte preflight for Solana, TON, and
  Substrate-family native recursive payloads before wallet/RPC envelopes are
  emitted, and transparent inner-proof plus native recursive package builders
  reject any bundle whose transparent public-input target domain is outside the
  verifier manifest's local/counterparty lane endpoints. The offline Solana
source-state evidence renderer now uses the same `finalized-vote` template
prefix as Rust and carries a golden vector for the template-hash rejection path.
Its direct material and source-adapter deployment record hash helpers now apply
that same template-hash rejection, so programmatic rollout tooling cannot
derive production-looking Solana evidence hashes from profile-template source
components. They also reject zero component, adapter verifier-key, deployment
receipt, and full-light-client audit hashes outside the CLI parser path, so
embedded rollout tooling cannot derive governed Solana records from empty
cryptographic evidence. Rust Solana source-material constructors now also
reject all-zero or template-derived AccountsDB source-state verifier hashes
before returning deployment-shaped material. Programmatic Solana source-state
evidence now also
requires exact `u32` source/target domain values, so boolean placeholders cannot
coerce to SORA domain `0` before material or deployment hashes are derived. It
also rejects non-canonical source-adapter OpenVerify
VK hashes before rendering governance TOML, so Solana operator evidence now
fails locally when it would fail Rust deployment matching. The compact JSON summary path now
uses the same Solana validation as the TOML renderer, so programmatic rollout
tooling cannot report partial Tower replay, full AccountsDB lattice, or
bank/fork-choice audit material as a valid dry-run. It now also reports
`source_verifier_material_ready`, `source_adapter_engine_deployment_ready`,
`source_adapter_gate_ready_with_full_light_client_evidence`,
`source_adapter_gate_blockers`, and `full_toml_ready`, so CI can gate source
pins separately from the complete source TOML predicate and report the exact
missing gate evidence. It now also accepts the
remaining Tower replay, full AccountsDB lattice, and bank/fork-choice verifier
hashes as an all-or-nothing audit bundle and emits a deterministic
`sccp:solana:full-light-client-gate:v1` hash over that bundle plus the
canonical source-material and deployment record hashes; production TOML rendering
now requires that complete audit bundle plus an independently supplied
`--expected-full-light-client-gate-hash`, so governed Solana audit records cannot
be staged from a self-derived gate hash alone or without deployed full-light
client evidence. When supplied, those hashes are now emitted as
source-adapter deployment config fields, appended to the canonical
`SccpSourceAdapterEngineDeploymentV1` record hash for Solana deployments,
  committed by the node ZK consensus policy hash, and recomputed by configured
  source-adapter parsing. The derived gate hash remains config/policy evidence
  instead of a deployment-record field to avoid circular hashing. `iroha_sccp`
  exposes the same transcript helper with a golden vector and now keeps the
  helper explicitly tied to the production Solana AccountsDB source-state
  verifier profile. The public source
  proof deployment-match helper now also requires the proof's embedded verifier
  evidence to carry the exact deployment record hash and deployment receipt hash,
  and re-verifies the adapter OpenVerify proof against that evidence so
  post-construction evidence splices or audited-bundle replays cannot be
  misreported as deployment-bound.
  JavaScript, Python,
  Swift, Kotlin, and Java Android now mirror the same Solana audit suffix in
  their canonical source-adapter deployment hash helpers, preserving the
no-audit hash while rejecting partial or non-Solana audit bundles before
UI-generated proofs submit on-chain. The same SDKs now also expose the
audited Solana full-light-client gate hash with the Rust golden vector, so
portals and mobile apps can audit the governed Tower replay, AccountsDB
lattice, and bank/fork-choice verifier bundle locally; those helpers also
reject duplicate audit verifier hashes and hashes that reuse existing
source-adapter material or built-in Solana template source-material component
hashes, and the Swift/Kotlin/Java Android gate-hash helpers now rerun that
check directly before returning a commitment. Rust admission and the offline
Solana source-state evidence renderer apply the same check before deriving the
full-light-client gate hash, so the governed audit suffix cannot be staged from
template verifier material through core, CLI, web, Python, Swift, Kotlin, or
Java Android callers. The ETH, BSC, Solana,
TON, and
Substrate-family source evidence renderers now also include the canonical
`SccpSourceVerifierMaterialV1` and
`SccpSourceAdapterEngineDeploymentV1` record hashes in compact JSON dry-runs
and TOML audit comments, matching `iroha_sccp` helper vectors before governed
evidence is copied into node configuration. Those renderers can now require
operator-supplied expected material/deployment record hashes before JSON or TOML
is emitted, so governance rollout scripts can fail on digest drift before
configuration is staged. Their direct material and source-adapter deployment
record hash helpers now apply the same template-hash rejection as the TOML
renderers, so programmatic rollout tooling cannot derive production-looking
ETH, BSC, Solana, TON, or Substrate-family evidence hashes from profile-template
source components. The ETH, BSC, Solana, TON, and Substrate-family source evidence
helpers now also require exact `u32` domain ids on their programmatic paths, so
Python boolean placeholders cannot be staged as SORA target-domain evidence.
The Solana submission helpers now require UI/mobile wrapped proof results to
match the explicit transparent SCCP message public inputs before wallet/RPC
submission, including message id, payload hash, commitment root, finalized slot,
and bank hash. They also re-derive the embedded Solana -> SORA source-adapter
deployment binding hash and require the source-proof public inputs to echo the
same deployment hash, receipt hash, and binding hash. They also recompute the
wrapped proof-result envelope hash and reject proof-byte overrides, so a valid
local proof cannot be paired with a different source-adapter deployment,
different on-chain message envelope, or different proof bytes through the SDK
convenience path. Swift, Kotlin/JVM, and Java Android now compare Solana
submission proof-context hashes, destination-binding hashes, transparent public
inputs, and message public inputs after canonical hex/slot normalization, so
mobile proof engines can echo equivalent request metadata without false
rejection while still failing on padded or mismatched fields.
JavaScript TypeScript declarations now also describe the full optional Solana
local-prover result metadata accepted by the runtime: canonical source public
inputs, source-state verifier ids/hashes, proof context, source-adapter
deployment binding, proof base64, and envelope hashes. Browser portal builds
can type-check the same UI prover contract that the wrapper revalidates before
submission.
The JavaScript and Python Solana source-state prover callbacks now also
validate optional structured OpenVerify/FastPQ result metadata against the
SDK-built request, including AccountsLtHash residual hashes, audit
role/verifier hashes, public input columns, FastPQ public inputs/transitions,
and statement/context/schema/commitment bytes. Browser and portal-backend proof
engines may echo equivalent camel/snake FastPQ aliases, numeric slots,
uppercase canonical hex fields, and byte/hex transition values, but
public-input columns remain exact. They therefore fail locally if they return a
proof capsule bound to a stale or padded source-state request transcript.
Structured source-state prover result version aliases are now also single-alias
checked and must normalize to `v1`, so proof engines cannot display one
`proofVersion` while the SDK wraps another version for submission.
The JavaScript package declarations now expose that flexible FastPQ result
metadata type for browser proof engines without widening the readonly
SDK-built request transcript types. The declaration requires one accepted
alias for each FastPQ root/hash and transition byte field, matching the runtime
single-alias guard.
Source-state proof capsule declarations now also require proof bytes and one
circuit-id alias, while structured prover result declarations require proof
bytes but keep circuit-id echoes optional, matching the request-bound runtime
callback behavior.
The same lane-aware helper validates structured TON source-state callback
metadata for TON role aliases, masterchain/shard seqnos, shard-state
public-input/proof hashes, public-input columns, FastPQ transitions, and
statement/context/schema/commitment bytes. The JavaScript declarations expose
that TON result object so portal builds see the same request transcript
contract that the runtime enforces.
Solana deployment-backed production admission now consumes the governed
full-light-client audit hashes as proof data, not only deployment metadata:
the source proof carries separate OpenVerify/FastPQ capsules for Tower replay,
full AccountsDB lattice, and bank/fork-choice verification. Production
verification with an audited Solana deployment rejects missing role capsules,
cross-role-spliced Tower replay, full AccountsDB lattice, or bank/fork-choice
capsules, tampered bank/fork-choice proof bytes, and proofs whose role verifier
hash no longer matches the exact deployment record.
The Rust Solana full-light-client audit role builders now also require the
nested AccountsLtHash OpenVerify/FastPQ capsule to verify cryptographically
against the governed AccountsDB source-state verifier before deriving any
second-stage audit role proof, so a shaped but invalid AccountsLtHash capsule
cannot be cascaded into Tower replay, full AccountsDB lattice, or
bank/fork-choice proof material.
Core bridge-proof admission now also covers the next gate: once an audited
Solana source proof opens the source-adapter gate, a replayed route allowlist
hash is rejected against the canonical source-material, source-deployment, and
destination-binding tuple. Taira's SCCP proof-size cap is sized to let those
audited Solana artifacts reach readiness validation.
The web, Python, Swift, Kotlin, and Java Android SDK surfaces now build the
three matching second-stage audit role proof requests from user UI material,
binding the Solana mainnet genesis public input, completed AccountsLtHash proof
hash, finality-context hash, vote-message hash, source verifier material,
deployment hash, gate hash, and role verifier hash before the proof is handed
to the local prover. The dynamic JavaScript and Python builders now reject
duplicate aliases in the AccountsLtHash source-state request and these
second-stage audit inputs before any FastPQ transcript is derived, including
blockhash/finality-context/vote-message aliases plus material, deployment, and
gate hash echoes. Those builders now reject empty or all-zero nested
AccountsLtHash proof bytes before deriving second-stage audit transcripts,
matching the Rust audit-capsule guard.
The typed Swift, Kotlin/JVM, and Java Android mobile inputs now also derive the
source material hash, source-adapter deployment hash, and full-light-client
gate hash from supplied source-material component hashes plus the deployment
receipt; any supplied precomputed hash is only an annotation and is rejected if
it diverges from the locally derived governed record.
The JavaScript and Python public source-state capsule canonicalizers now match
the mobile/Rust profile checks as well: Solana canonicalization only accepts
the AccountsLtHash `stark-fri-v1` circuit, and explicit TON canonicalizers only
accept the shard-state `stark-fri-v1` circuit before hashing completed proof
capsules for UI prover output. JavaScript FastPQ source-state and
full-light-client audit proof request builders for Solana, TON, and
Substrate-family runtime-storage proofs now freeze the returned request objects,
public-input columns, transition metadata, and aggregate request maps, and
return canonical statement, context, schema, commitment, and witness byte fields
through defensive-copy getters, preventing browser code from mutating a
UI-visible proof request after transcript derivation. Solana and TON
full-light audit requests across JavaScript, Python, Swift, Kotlin, and Java
Android now expose the same snake_case role ids to linked web/mobile prover
callbacks (`tower_replay`, `full_accountsdb_lattice`, `bank_fork_choice`,
`masterchain_config`, `validator_set_transition`, and
`shard_accounts_dictionary`) while preserving language-native aggregate result
properties.
The TypeScript
declarations mark those request objects, nested arrays, transition entries, and
aggregate maps as readonly for portal compile-time checks. Python portal-backend
builders now return the same Solana, TON, and Substrate-family FastPQ requests
as read-only dict/list-compatible envelopes with immutable byte payloads,
preserving normal inspection while preventing callback-side metadata rewrites.
Kotlin mobile request models now also store defensive copies and return fresh
byte arrays for Solana AccountsLtHash, Solana/TON full-light audit, and
Substrate-family runtime-storage request bytes and FastPQ transition byte values,
matching the Java Android record/accessor surface. Java Android also freezes the
Substrate-family runtime-storage public-input and transition lists, and Swift
tests now pin the same value-snapshot contract for mobile proof requests.
The Swift, Kotlin, and Java Android direct audit-request inputs now also reject
duplicate audit role verifier hashes and reuse of any source-adapter material
role hash, including trust-anchor, consensus, message-inclusion,
finality-policy, source-state verifier, adapter verifier-key, and deployment
receipt material, before handing work to a mobile prover. Those direct mobile
inputs also recompute the Solana full-light-client gate hash from the
source verifier material hash, source-adapter deployment hash, and role verifier
hashes, so a UI cannot hand the prover a stale or unrelated gate commitment.
JavaScript and Python Solana full-light audit request construction now performs
the same role-separation preflight before deriving browser/portal prover
transcripts, including the guard against reusing governed source-state or
deployment material as an audit role verifier hash. Web/Python regressions now
also pin that supplying a source-adapter deployment record without matching
witness deployment hash/receipt fields fails before a user-linked prover can
produce request-bound proof bytes, and Swift/Kotlin/Java Android tests cover
the same stale witness deployment-hash and receipt-hash paths for mobile proof
UIs.
Python Solana witness and deployment-binding normalizers now preserve explicit
empty strings through validation, matching JavaScript's rejection of empty
source-state verifier and source-adapter deployment binding fields instead of
silently treating them as absent.
JavaScript, Python, Swift, Kotlin, and Java Android also now wrap completed
Solana and TON OpenVerify/FastPQ proof bytes into checked source-state proof
capsules pinned to the originating AccountsLtHash, TON shard-state, or
full-light audit request circuit, giving web portals, portal backends, and
mobile apps a request-bound output step before those proof capsules feed the
next audit/source proof stage. Solana canonical source-state capsule bytes now
cover the AccountsLtHash circuit and the three full-light audit circuits, while
the AccountsLtHash proof-hash helper remains restricted to the nested
AccountsLtHash circuit. Solana wrappers now rederive the canonical
FastPQ transition bindings for the AccountsLtHash request and each full-light
audit role, rejecting externally generated proof bytes if any transition key,
operation, old value, or new value no longer matches the SDK-built request
transcript.
TON wrappers now rederive the canonical FastPQ
transition bindings for the shard-state request and each full-light audit role
and reject externally generated proof bytes if any transition key, operation,
old value, or new value has been mutated. Those SDKs now expose app-linked
source-state prover facades as well: web/Python callers can inject a prove
callback for SDK-built Solana AccountsLtHash, TON shard-state, and
role-separated full-light audit requests, while Swift, Kotlin, and Java Android
mobile apps can inject
typed AccountsLtHash, TON shard-state, and audit-role proof engines. The facades
invoke the linked prover only after the request has passed the same request-shape
checks used by the wrap helpers, and the direct Solana full-light audit builders
now reject request-bound role verifier hash reuse before returning requests to a
web portal or mobile app. The TON direct audit request builders now apply the
same separation to source-state proof hashes, shard-state public-input hashes,
deployment/material/gate hashes, role columns, and audit-statement hashes, so a
governed TON role verifier hash cannot be replayed from per-request transcript
material before UI proof generation starts.
TON validator-set transition-chain hashes now commit to the complete canonical
transition proofs across Rust, JavaScript, Python, Swift, Kotlin, and Java
Android, including BlockIdExt fields, next-set payload bytes, signer bitmap,
validator keys/weights, and signature bytes. This makes the shard-state and
full-light audit request transcripts change when a UI/RPC witness mutates any
transition field, not only when the summary hashes change. Web, Python, Swift,
Kotlin, and Java Android regressions now include non-empty transition lists so
portal and mobile proof-generation paths cover the production transition-chain
case instead of only the trust-anchor-direct fixture path.
The same web, Python, Swift, Kotlin, and Java Android source-material helpers
now reject all deterministic Solana template component hashes, including source
trust-anchor, consensus, message-inclusion, AccountsDB source-state, and
finality-policy hashes, so UI provers cannot derive production request
transcripts from placeholder verifier material.
Rust Solana full-light audit proof builders and verification now also reject
governed role verifier hashes replayed from request-bound source-state,
material, deployment, gate, finality, vote-message, nested AccountsLtHash proof,
or audit-statement hashes, so an otherwise ready deployment cannot package or
admit an audit proof whose verifier identity is borrowed from the live request
transcript.
Solana source-adapter structural admission also requires the signer bitmap to
use the exact byte width implied by the validator roster and to keep unused
padding bits zero before vote-proof verification or transcript hashing. The
same structural preflight now rejects present source-state proof capsules unless
they are version `1`, use `stark-fri-v1`, carry a non-empty circuit id, and
contain non-empty/non-all-zero proof bytes before Norito/OpenVerify decoding,
covering nested AccountsLtHash material, full-light-client audit role proofs,
TON source-state capsules, and Substrate-family runtime-storage capsules. The
Solana AccountsLtHash verifier and role-separated full-light-client audit
verifier now apply the same all-zero capsule guard before decoding, so direct
verifier calls cannot bind placeholder proof bytes outside structural admission.
For the web/mobile portal path, the
remaining Solana blocker is governed all-lanes rollout with live
full-light-client verifier deployments, not request derivation or the
source-adapter production predicate itself. The all-lanes preflight now rejects
Solana destination rollout evidence that was not produced with live immutable
ProgramData metadata, so the remaining rollout blocker is governed deployment
and canary evidence for those live verifier programs. That metadata now pins a
positive expected ProgramData slot alongside the ProgramData address and
executable hash, the base64 ProgramData executable preimage, plus finalized RPC
  context-slot metadata proving the ProgramData account was read at or after the
  deployment slot, requires the canonical 36-byte Program account layout, and
  now carries base64 preimages for that Program account and the immutable
  ProgramData metadata header. The all-lanes gate decodes those preimages to
  prove the Program account points to the claimed ProgramData address and the
  ProgramData header encodes the same deployment slot with no upgrade authority.
  It rejects ProgramData metadata that aliases the verifier program id, carries a
  non-BPF-ELF executable preimage, or tampers with either account/header preimage,
  so confirmed, placeholder, self-referential, stale slot, wrong-layout,
  non-executable-byte, or hash-only comments cannot satisfy all-lanes readiness.
  Those ProgramData pins now render as configured `solana_*` rollout fields, not
  only helper comments, and Rust/Core route readiness recomputes the Solana
  canary evidence from that configured ProgramData transcript before accepting a
  route allowlist for production. Generic non-zero canary hashes are no longer
  sufficient for the SORA -> Solana lane.
  The Solana live evidence direct APIs also
  revalidate imported live dictionaries for BPF-loader owners, immutability,
  canonical Program preimages, immutable ProgramData metadata bytes, fresh slots,
  executable length, and executable/hash consistency before reporting
  `toml_ready` or rendering TOML, so forged live metadata cannot bypass JSON-RPC
  collection. The
direct Solana destination helper and its importable render/summary APIs can
derive the same verifier code hash from supplied program bytes and reject
mismatches with an explicit hash before producing JSON or TOML. Production
TOML now rejects hash-only Solana destination evidence and JSON summaries keep
`toml_ready = false` until the same BPF ELF executable preimage is supplied, so
copied verifier hashes cannot satisfy the direct rollout path. It now applies
the same
launch posture to TON destination rollout evidence by requiring live active
account-state metadata from the TON live helper, so the remaining TON release
blocker is governed canary evidence for the live verifier contract and full
lane rollout. The offline
`scripts/sccp_ton_source_state_evidence.py` helper now renders the governed
TON -> SORA source material and source-adapter deployment TOML from
operator-collected live component hashes plus the required TON full-light-client
audit verifier hashes. It rejects the template-derived TON source trust-anchor,
consensus-verifier, message-inclusion, source-state verifier, and
finality-policy hashes before rendering governance TOML, appends the audited
deployment suffix, and emits the runtime-checked
`ton_full_light_client_gate_hash`, so the remaining TON release blocker is
governed live deployment/canary evidence and lane rollout rather than SDK
request binding or source-adapter admission wiring. The helper and its direct
hash APIs now require exact `u32` source/target domain values, so boolean
placeholders cannot coerce to SORA domain `0` before source material,
deployment, or full-light-client gate hashes are derived. They also reject
padded component hashes and source/target domain values before those record and
gate hashes are derived. Its compact JSON summaries
now also expose `source_verifier_material_ready`,
`source_adapter_engine_deployment_ready`,
  `source_adapter_gate_ready_with_full_light_client_evidence`,
  `source_adapter_gate_blockers`, and `full_toml_ready`, matching the Solana
  source-state diagnostics so CI can distinguish pinned TON material, pinned
  source-adapter deployment, complete full-light-client TOML readiness, and the
  exact missing gate evidence. The all-lanes JSON summary now also emits a
  per-lane `source_adapter_gate` object with `required`, `ready`, `gate_hash`,
  `audit_hashes`, and `blockers`, so Solana/TON rollout automation can read the
  recomputed full-light-client gate state directly from the final launch
  preflight rather than parsing generic lane blockers. Rust source proofs now
  carry the three TON full-light-client audit capsules as proof data, and
  deployment-aware verification rejects missing masterchain-config capsules,
  spliced validator-set-transition capsules, tampered shard-accounts dictionary
capsules, all-zero shard-state or audit-role proof capsules, duplicate audit
verifier hashes, audit hashes that reuse existing verifier material, and audit
hashes that reuse built-in TON template component hashes. Core bridge-proof
admission now has focused coverage that
configured audited TON source deployment material reaches the all-lanes gate,
while mismatched or partial TON full-light-client audit records fail before
structural proof evaluation. The SDK source-material and shard-state
proof-request builders now reject those template-derived TON
component hashes, including the shard-state source-state verifier hash, before
invoking the app-linked prover. Rust TON source-material constructors now also
reject all-zero or template-derived shard-state source-state verifier hashes
before returning deployment-shaped material. The helper also rejects
non-canonical source-adapter OpenVerify VK hashes before rendering governance
TOML, matching the Rust deployment-material gate. The Rust SCCP crate and
JavaScript, Python, Swift, Kotlin, and Java Android SDKs now also build the
exact TON shard-state
source-state OpenVerify/FastPQ request from UI/mobile witness material,
including statement bytes, witness commitment bytes, verification context,
schema descriptor, public-input columns, FastPQ metadata transitions, and the
`sccp-ton-shard-state-light-client-v1`/`fastpq-lane-balanced` identifiers. The
shared vector now uses the production-consistent masterchain config proof hash
for the same `ShardStateUnsplit` root that the TON shard-state opening proves,
matching the Rust FastPQ batch gate. The
`scripts/sccp_ton_destination_evidence.py` helper now covers the SORA -> TON
destination rollout and route allowlist records from the deployed TON verifier
contract address and a code hash derived from the single-root verifier code
BoC, and it recomputes the route allowlist hash from the governed TON
source-material record hash, audited source-adapter deployment record hash, and
canonical SORA -> TON destination binding. The live helper recomputes the
returned `code_boc` root hash before trusting TON Center's `code_hash` and
keeps the actual code BoC in the generated TOML comments so all-lanes can
replay the root-hash derivation. The offline
`scripts/sccp_all_lanes_evidence.py` preflight consumes the rendered source,
destination, and route TOML snippets and reports lane-specific blockers for any
missing or non-production record across ETH, BSC, Solana, TON, TRON, and the
	Substrate-family lanes before governance staging. It also rejects unsupported
	domain records in the source-material, source-adapter deployment, destination
	rollout, and route-allowlist sections instead of ignoring stray governance
	records while declaring the advertised lanes ready. It also recomputes the
	Solana and TON full-light-client gate hashes and TRON source bridge config hash,
	so governance cannot stage arbitrary non-zero audit placeholders,
	template-derived component hashes, non-canonical source-adapter verifier keys,
	reused non-zero source/deployment/audit role digests, or malformed destination
		verifier identities. Native Solana, TON, and
		Substrate-family destination evidence JSON now stays binding-only until the
	expected destination binding hash and route canary evidence are pinned; route
	allowlist hashes and paired source record hashes are rejected before that
	independent binding pin matches. TON
source TOML now matches the
Solana audit-gate discipline by requiring all three TON full-light-client audit
verifier hashes plus an independent `--expected-full-light-client-gate-hash`
before TOML is rendered; JSON summaries stay diagnostic and report
`toml_ready = false` until that complete audit bundle and pin are present. The
TON audit gate now also rejects audit hashes that reuse built-in template
component hashes before deriving production-looking gate hashes, and the
JavaScript, Python, Swift, Kotlin, and Java Android SDK proof-request builders
apply the same rejection before UI/mobile prover invocation. For ready lanes
the same JSON summary reports canonical source material and source-adapter
deployment record hashes for governance comparison. For
Substrate-family destination lanes,
JavaScript, Python, Swift, Kotlin, and Java Android portal/mobile SDKs
now build request-bound `substrate-runtime-v1` prover inputs and envelope
hashes for SORA-Kusama, SORA-Polkadot, and SORA2, so the remaining Substrate
release blockers are governed runtime verifier deployment evidence and lane
rollout rather than SDK request derivation for either destination runtime
proofs or source-state runtime-storage proofs. Release summaries now also carry
the derived `substrate_runtime_storage_gate_hash` as a required source-adapter
audit hash for those lanes. The direct Substrate-family
destination helper and its importable render/summary APIs can now derive the
runtime verifier code hash from supplied runtime bytes and reject explicit hash
mismatches before producing JSON or TOML. Rust runtime-storage verification
also rejects all-zero source-state proof capsules before OpenVerify/FastPQ
decode, matching the Substrate source-adapter preflight. For TRON, extend the current
signed ancestor-linked solid-block header proof,
  which now authenticates both `txTrieRoot` and TRON `accountStateRoot`, witness
  seal, witness-schedule transition certificates, and transaction-Merkle
  source-call proofs over authenticated transaction bytes. Deployment-backed
  TRON source-adapter readiness can open when exact mainnet source material,
  governed source-bridge contract evidence, adapter verifier commitment, and
  deployment receipt hashes match; the mainnet source-material identity now
  uses the `transaction-source-mainnet` verifier profile and commits to the
  transaction source-call transcript prefix as well as the legacy structural
  receipt prefixes, and TRON recoverable signature preflight
  rejects invalid `r` scalars before hashing header or witness-seal material.
  Witness-schedule transition block hashes must be backed by the supplied
  solid, parent, or signed ancestor header evidence before a schedule rotation
  can activate.
  The TRON transaction source-call verifier now requires
  `contractRet = SUCCESS`; explicitly present top-level `ret` values must be
  java-tron's default `SUCESS = 0`, while canonical transactions that omit that
  default field are accepted. Only the canonical optional `Result.fee` field
  may accompany those success fields, and unknown result extensions fail closed.
  JavaScript, Python, Swift, Kotlin, and Java Android transaction-source hash
  helpers now mirror that omitted-default-ret rule against the same canonical
  proof vector.
  The TRON source bridge evidence direct material and
  deployment record hash helpers now reject template-derived source component
  hashes, matching the TOML renderer so programmatic rollout tooling cannot
  derive production-looking records from template material. The same SDK helpers
  now recompute the TRON source bridge config hash from the bridge address,
  network id, TRON -> SORA lane ids, and owner address, rejecting mismatched
  caller-supplied config hashes and padded fixed-width evidence strings before
  deriving production-looking records. The
  authenticated source-call transaction must now carry exactly one recoverable
  signature, and that signer must be the configured source-bridge owner, so
  duplicate, multisig, or non-owner signatures cannot pad the source-call
  transaction; Python, JavaScript, Swift, Kotlin, and Java Android source-proof
  helpers now run the same recovery check before UI/mobile transcript hashing.
  The outer
  `Transaction` parser, the signed `raw_data` parser, and the nested
  `Transaction.Contract`, `Any`, and `TriggerSmartContract` parsers also reject
  unknown fields, forcing future java-tron call extensions to fail closed until
  they are explicitly profiled and bound. The signed `raw_data` parser now also
  requires non-zero ref-block bytes/hash, non-zero expiration/timestamp/fee limit,
  and `expiration > timestamp`, while keeping deprecated `ref_block_num`
  optional. The Rust source-call calldata helper
  and source-adapter deployment builder are now
  locked to the same production TRON -> SORA lane as the SDK helpers and reject
  non-TRON sources, non-SORA targets, and zero source-event digests before
  encoding calldata or deployment evidence. Rust and the JavaScript, Python,
  Swift, Kotlin, and Java Android SDK transaction-source helpers now reject any
  `transaction_root` that does not recompute from
  `transaction_bytes`, `transaction_index`, `transaction_count`, and
  `transaction_merkle_branch`, and the SDK helpers reject non-canonical TRON
  recoverable transaction signatures before deriving production-looking
  source-proof hashes. Those SDK helpers now also accept optional governed
  source bridge emitter/owner address expectations and reject transaction bytes
  whose embedded `TriggerSmartContract` contract or owner address drifts from
  the production source material before deriving those hashes. Rust now exposes
  matching source-bridge-bound transaction-source bytes/hash helpers for
  operator tooling; matching governed material preserves the existing canonical
  transcript, while wrong bridge/owner or zero-address pins fail before any hash
  is returned. Rust also exposes material-bound variants that extract those pins
  from production-ready TRON source verifier material so operators do not have
  to copy bridge and owner addresses by hand. When that transaction-source proof
  is present,
  the legacy receipt proof fields must remain canonical-zero/empty so unused
  receipt-index or trie material cannot be carried through production adapter
  transcripts. The Rust TRON verifier now also rejects non-canonical protobuf
  varints before parsing source-call transactions, raw header data, result
  messages, or legacy receipt logs, preventing overlong or overflow-shaped
  encodings from becoming alternate accepted transcripts. JavaScript, Python,
  Swift, Kotlin, and Java Android solid-block header helpers now decode those
  raw header fields with the same canonical varint rule and recompute the
  supplied raw-data hashes, block ids, parent link, trie roots, witness address,
  timestamp, and header version before returning transcript bytes.
  The live evidence step now has an offline helper,
  `scripts/sccp_tron_source_bridge_evidence.py`, that recomputes the governed
  source bridge config hash and renders the matching source material,
  source-adapter deployment, destination rollout, and route allowlist TOML from
  collected deployment hashes, including the source bridge network id and owner
  address bound by production admission. TRON verifier evidence now recomputes
  that source bridge config hash whenever the config fields are populated, so
  mismatched material-only evidence fails before production deployment gates are
  considered. The paired read-only live collector,
  `scripts/sccp_tron_live_evidence.py`, queries deployed source bridge and
  destination verifier view functions through TRON constant calls, recomputes
  `sourceBridgeConfigHash()` and `destinationBindingHash()`, optionally
  cross-checks `/wallet/getcontract` bytecode metadata, supports
  `/walletsolidity/triggerconstantcontract` confirmed-state reads, supports a
  runtime-only `TRON-PRO-API-KEY` header for TronGrid endpoints, requires each
  constant-call response to carry an explicit successful `result.result = true`
  flag before trusting the returned ABI word, and emits diagnostic
  offline-renderer arguments plus the Torii/CLI destination-query fields for
  artifact/job submission without signing, broadcasting, deploying, or mutating
  chain state. Those offline arguments only carry expected source-config and
  destination-binding pins after the corresponding operator-supplied expected
	  values match the live views, and they only carry the route allowlist hash after
	  the expected destination-binding pin matches; the live collector now rejects
	  supplied route allowlist evidence before that pin matches instead of reporting
	  a route hash in diagnostic JSON. They now carry route canary evidence only
	  after that route tuple is verified, and live full-TOML output stays disabled
	  until the same route canary hash is present. When supplied with governed source component hashes,
  the source-adapter deployment receipt hash, and expected source record hashes,
  the live collector also recomputes the canonical TRON source verifier material
  and source-adapter deployment hashes before emitting offline rollout arguments;
  any mismatch fails in the read-only collection step. When metadata lookup is
  enabled, source-bridge live collection requires `/wallet/getcontract`
  source-bridge bytecode to be present and bound to the queried contract address
  so a manual source code hash cannot fill a missing or cross-contract node
  observation; destination metadata applies the same address binding before
  verifier bytecode is trusted. The live collector and direct renderer now
  preserve both runtime bytecode preimages in TOML/offline arguments, and
  all-lanes recomputes the source bridge and destination verifier code hashes
  from those bytecode comments before launch readiness. Operators must pass
  `--no-getcontract` only after an independent code-hash audit. Operators can also pin the deployment/governed
  source bridge config hash with
  `--expected-source-bridge-config-hash`, so owner or lane drift is rejected
  before production full-TOML output; diagnostic offline arguments include that
	  expected pin only after it matches, and route allowlist/canary arguments remain
	  withheld until the destination binding is also independently pinned. Direct
	  full-TOML output requires that config pin plus complete
	  `--route-canary-transaction-*` metadata, and the direct TOML renderers now apply the
	  same runtime-bytecode hash derivation/mismatch checks as the CLI before
	  emitting governance records. Direct full-TOML output now also annotates the
	  source bridge and destination verifier metadata required by all-lanes
	  preflight, so independently audited TOML can satisfy the same launch gate as
	  live-collected evidence. Live full-TOML output also requires an
	  explicit governed `--expected-destination-binding-hash` match,
	  transaction-derived route canary evidence, and `/wallet/getcontract`
	  bytecode metadata for both the source
	  bridge and destination verifier before the destination rollout can be emitted.
	  The live helper now requires a transaction-derived route canary for live
	  production TOML and derives that hash from a supplied
	  `--route-canary-transaction-id` by verifying the destination verifier's
	  `MessageProofAccepted` log against the deployed binding/backend/family/network
	  views, exact transaction-info `blockNumber`/`blockTimeStamp` metadata, and
	  the validated route allowlist hash, then fetching the raw
	  `TriggerSmartContract` transaction, parsing the hashed `raw_data_hex`,
	  requiring its owner address to match the visible transaction owner,
	  requiring the single canonical recoverable secp256k1 signature to recover
	  to that owner, and checking the
	  `submitSccpMessageProof(bytes,bytes32[6],bytes32)` selector, ABI public
	  inputs, statement hash, 384-byte proof tuple, and proof header against the
	  accepted event and deployed verifier domains, plus
	  `usedMessageProofs(messageId)` current-state consumption; if operators also supply
	  `--route-canary-evidence-hash`, it must match the transaction-derived value.
	  All-lanes preflight now requires those TRON canary transaction fields and
	  audit comments for TRON, including the `usedMessageProofs` state result,
	  `tron_route_canary_transaction_owner_address`,
	  `tron_route_canary_raw_data_owner_matches_transaction`, the route-canary
	  signature hash, recovered address, and `signature_recovers_to_owner` audit
	  result. The recovered address must equal the transaction owner before the
	  gate recomputes the canary evidence hash and treats the route evidence as
	  bound, and saved full-TOML replay revalidates the carried route-canary block
	  metadata before offline arguments can be regenerated. All-lanes and the
	  direct renderer also reject reuse across distinct
	  TRON canary transcript hash roles, including transaction id, message id,
	  calldata, payload, statement, commitment, finality height, finality block,
	  and signature hash fields. The canonical TRON canary transcript now uses
	  `iroha:sccp:tron-route-canary-evidence:v3` and commits the exact
	  `submitSccpMessageProof(...)` calldata SHA-256, decoded payload hash,
	  target-domain word, finality-height word, finality block hash, proof
	  version, proof source domain, transaction owner, route-canary block number,
	  route-canary block timestamp, raw-owner binding flag, signature SHA-256,
	  recovered signer, and recovery flag in the evidence hash itself; Rust lane
	  readiness mirrors that transcript and hash-role
	  separation gate from the first-class
	  `tron_route_canary_*` route record fields, and the configured Torii/Core
	  all-lanes path preserves the owner/signature, transaction block metadata,
	  and v3 call-transcript fields into launch readiness. ZK consensus policy
	  hashing now commits those TRON route-canary fields and the existing EVM
	  route-canary transaction fields.
	  The direct TRON renderer now
	  requires the same `--route-canary-transaction-*` metadata plus
	  `--route-canary-used-message-proof` and
			  `--route-canary-raw-data-owner-matches-transaction` plus the
			  `--route-canary-signature-*` recovery metadata for full TOML, requires the
			  recovered address to match `--route-canary-transaction-owner-address`,
			  derives the canary hash when an explicit hash is omitted, and rejects
			  transaction/hash mismatches before emitting full TOML. Saved live JSON
			  replay now also revalidates the route-canary selector, Groth16 tuple
			  length/version/source domain, public-input message id, target domain,
			  payload hash, commitment root, finality words, statement hash,
			  owner/signature fields, submitted calldata hash, and recomputed canary
			  hash before allowing offline production TOML.
		  Swift, Kotlin, Java Android, and the EVM/TRON contract smoke now validate
		  the local TRON proof request, proof wrapping, verifier-call packaging, and
		  wrapper/source-bridge contract paths; the remaining TRON production
		  checkpoint is governed live deployment plus route-canary evidence, not
		  local SDK or contract wiring. Rust deployment-backed TRON source-adapter
		  readiness now also derives the typed `sccp:tron:dpos-source-gate:v1`
		  transcript over the source material, source-adapter deployment, adapter
		  VK, DPoS/witness/source-call role hashes, governed source bridge config,
		  and TRON verifier prefixes/bounds before the source-adapter gate opens.
		  That gate is now first-class configured evidence as
		  `tron_dpos_source_gate_hash`: TRON source-adapter deployments must carry
		  the exact recomputed hash through Torii/core configured-lane admission,
		  the ZK consensus policy hash, the direct/live TRON evidence renderers,
		  and all-lanes preflight; non-TRON lanes must leave it empty. Production
		  TRON source/full-TOML rendering and live full-TOML readiness now also
		  require the operator-supplied
		  `--expected-tron-dpos-source-gate-hash` to match that canonical gate,
		  so JSON dry-runs can derive the hash but rollout TOML cannot become ready
		  until the first-class DPoS/source-call gate is explicitly pinned. Direct
		  `--full-toml` now also requires source-bridge and destination-verifier
		  runtime bytecode preimages, keeping hash-only direct CLI runs diagnostic
		  instead of production-ready.
		  `--no-getcontract` remains a
	  diagnostic JSON path only; it cannot produce production-ready full TOML.
  When the verified live
  evidence is sufficient for full governance TOML, the collector emits
  `offline_full_toml_args` with `--full-toml` already appended, verifies that
  the generated argument list renders through the offline governance TOML
  helper, emits `offline_full_toml_sha256`, and can print that verified TOML
  directly with `--full-toml`. The all-lanes rollout preflight now has a
  regression that accepts this verified live TRON full-TOML output when merged
  into a complete SCCP evidence bundle and checks the resulting TRON source
  record hashes against the live collector output. The public full-TOML renderer
  now also requires those computed source-record match flags in the summary, so
  hand-built summaries with only expected-hash strings cannot bypass live hash
  verification. The all-lanes gate also
  rejects TRON records that lack live source-bridge or destination-verifier
  address/code-hash metadata, so offline/manual TRON records remain diagnostic
  instead of launch-ready. The same live
  pass now fails if destination verifier bytecode metadata is missing or
  disagrees with the deployed `verifierCodeHash()` view, and the public
  full-TOML argument helper now withholds `offline_full_toml_args` unless the
  live metadata match flag is present and the two destination code-hash strings
  are identical. Live destination evidence now also checks
  `verifierBackendHash()` and `proofFamilyHash()` against the canonical
  `tron-groth16-bn254-v1` / `stark-fri-v1` deployment profile before emitting
  rollout or Torii query material, carries those hashes into full-TOML metadata,
  and the all-lanes preflight requires the metadata to match the canonical
  profile. Torii destination query parameters now also require the destination
  `/wallet/getcontract` bytecode hash to match `verifierCodeHash()`, so
	  signer-free artifact/job material cannot be emitted from `--no-getcontract`
	  diagnostics, and the live helper now re-parses and recomputes the destination
	  binding from summary fields before exposing importable Torii query params,
	  using strict canonical u32 parsing so boolean or leading-zero summary domains
	  cannot be coerced into a production lane.
	  Those Torii/CLI fields
  now include `expected_destination_binding_hash_hex` only after the
  operator-supplied `--expected-destination-binding-hash` matches the live
  verifier view, and Torii now requires that pin for EVM/TRON artifact or job
  requests whenever deployment destination fields are supplied, then rejects the
  request when the live binding hash does not match the binding recomputed from
  those deployment fields. The live summary also marks those query params as
  requiring external `proof_bytes_hex`, so deployment evidence is not mistaken
  for a complete artifact/job request. When a destination rollout
  must also be configured on the node; Torii rejects deployment-bound artifact,
  job, bridge-proof, and bridge-message requests when the rollout is missing or
  when the recomputed destination binding key/hash does not match the configured
  rollout. JavaScript and Python clients expose the
  same guard. JavaScript, Python, Swift, Kotlin, and Java Android
  source-material/deployment hash helpers now apply the same TRON template
  component rejection before emitting production-looking hashes. Source bridge
  ownership transfers also emit the new config hash
  automatically, giving rollout evidence a direct owner-rotation audit event.
  The same helper recomputes the TRON destination Groth16 binding
  hash from the base58 wrapper address, deployment hashes, proof family, network
  id, and source/target domains, requires `--expected-config-hash` for
  production source TOML, can compare
  `--expected-source-verifier-material-hash` and
  `--expected-source-adapter-engine-deployment-hash` plus the governed
  `--expected-tron-dpos-source-gate-hash` before JSON, source TOML, or full
  rollout TOML output, requires both expected source record hashes, the expected
  DPoS source-gate hash, and `--expected-destination-binding-hash` for full
  TOML, derives
  source/destination runtime code hashes from deployed bytecode when supplied,
  derives and verifies the canonical TRON -> SORA
  `adapter_verifier_vk_hash` from the FastPQ/OpenVerify source-adapter verifier
  profile, requires both expected source-record hashes and the DPoS source-gate
  hash for production source TOML while still allowing unpinned diagnostic JSON,
  includes the canonical source material, source-adapter deployment record, and
  DPoS source-gate hashes in complete JSON dry-runs and TOML audit comments,
  and includes the recomputed destination value plus canonical
  `SccpDestinationBindingV1.key` in JSON/full TOML output. The helper's compact
  JSON, source TOML, and full rollout TOML output modes are mutually exclusive,
  and the source/direct full-TOML regressions now pin the destination rollout to
  a single `verifier_code_hash` key so strict TOML parsers do not reject
  governance bundles; the all-lanes fallback TOML loader now rejects duplicate
  keys as well when `tomllib` is unavailable. Its production TOML modes are
  locked to the
  TRON -> SORA source lane so operators cannot accidentally render
  non-admissible records for another target domain. Full rollout TOML is also
  locked to the paired SORA -> TRON destination binding and `stark-fri-v1`
  proof family, and compact JSON applies those same source and destination lane
  checks before returning hash dry-runs. Compact JSON also treats
  `--route-allowlist-hash` as destination-side evidence, rejects route-only
  partial summaries without the paired destination verifier material, and emits
  the route hash only when that destination material is complete. Compact JSON
  now reports `full_toml_ready = true` only after the source TOML pins,
  destination binding pin, route allowlist hash, and route canary evidence are
  all present and matched, and marks unpinned destination material with
  `expected_destination_binding_hash_matches = false`. Compact JSON also accepts
  `--source-event-digest` and emits the exact
  `submitSccpSourceEvent(uint32,uint32,bytes32)` owner-call calldata for the
  TRON -> SORA source bridge plus the unsigned `/wallet/triggersmartcontract`
  request body, while TOML modes reject that one-off payload so governance
  evidence stays separate from transaction execution material. The
  live collector mirrors that JSON-only source-event behavior for queried source
  bridges, emits `offline_source_event_args` for reproducible direct-helper
  calldata, rebuilds those replay arguments from the saved source bridge
  domains, owner, digest, and calldata instead of trusting stored replay arrays,
  checks `submittedSourceEvents(bytes32)` before pre-submit trigger rendering,
  includes an unsigned `/wallet/triggersmartcontract` request body
  for fresh digests, verifies a post-submit transaction id against the successful
  exact two-topic `SccpSourceEvent(bytes32)` log and raw TriggerSmartContract
  ret/owner/contract/calldata tuple, requires `gettransactioninfobyid` and
  `gettransactionbyid` transaction-id aliases (`id`, `txID`, and `txid`) to
  agree, requires raw transaction readback to carry canonical `txID`, requires
  the transaction `raw_data_hex` to hash to that `txID`, requires exactly one
  canonical 65-byte TRON recoverable secp256k1 signature that recovers to the
  source bridge owner,
  parses the signed `raw_data_hex` protobuf for the same
  owner/contract/calldata/ref-block/timing/fee source-call profile that Rust
  production admission checks, requires exact source-event transaction-info
  `blockNumber` and `blockTimeStamp` metadata and cross-checks the timestamp
  against the fetched block header, requires saved replay JSON to retain that
  same block metadata/solid-block binding, rejects boolean/truthy source-event block
  numbers, timestamps, header versions, and signed-header depth arguments before
  source-event evidence is summarized, uses solidity transaction readback
  endpoints when `--solid` is set, keeps `full_toml_ready` visible on JSON dry-runs, and
  rejects `--source-event-digest --full-toml`. Post-submit JSON also emits
  canonical transaction protobuf bytes, the SHA-256 transaction hash, and the
  transaction Merkle branch; when `--receipt-root` plus repeated
  `--source-inclusion-branch-hex` values are supplied, it derives canonical
  `sccp:tron:transaction-source-proof:v1` bytes/hash and compares
  `--receipt-proof-hash` when present. It fetches the containing block,
  rebuilds the canonical block-header `raw_data` hash/TRON block id, fetches
  the immediate parent block, verifies the child `parentHash` and monotonic
  timestamp, recovers both child and parent header signatures to their declared
  TRON witness addresses, and recomputes java-tron's transaction Merkle root
  from the block's canonical transaction protobuf bytes whose
  `txID`/`txid`/`id` aliases agree to match `txTrieRoot`;
  that live block reconstruction now serializes market-order
	  `orderDetails` and stake-v2 `cancel_unfreezeV2_amount` result extensions on
	  unrelated block transactions instead of rejecting otherwise usable source
	  evidence, and its integer parser accepts JSON numbers or decimal strings
	  while rejecting booleans, non-ASCII or leading-zero decimal strings, and
	  values outside non-negative `int64` bounds. The source-event success checks
	  also accept canonical enum names or numeric protobuf JSON values (`ret = 0`,
	  `contractRet = 1`) while rejecting the non-canonical top-level `ret =
	  SUCCESS` alias plus non-ASCII or leading-zero numeric enum strings, and
	  source-inclusion branch siblings are allowed to be any canonical 32-byte value, including
	  all-zero. Live block-header reconstruction now rejects all-zero
	  `0x41`-prefixed witness addresses before deriving raw-header bytes, matching
	  the Rust and SDK raw-header parsers. Rust, Python, JavaScript, Swift, Kotlin,
	  Java Android, and live
	  collector witness schedule payload helpers now also fail closed when the sum
	  of non-zero witness weights cannot fit the `u64` total committed by witness
  seals;
  when child and parent account-state roots are present it also emits the
  canonical solid-block header proof bytes/hash, while missing roots remain a
  visible blocker. It also accepts canonical witness-schedule payload hex or a
  payload file, derives the `sccp:tron:witness-schedule:v1` hash, optionally
  checks an expected schedule hash, and requires child/parent block witnesses
  to be active schedule members. It now also accepts receipt-root,
  receipt-proof-hash, witness-signers bitmap, and repeated witness-signature
  inputs, derives canonical `sccp:tron:solid-block-message:v1` and
  `sccp:tron:witness-seal:v1` bytes/hashes, verifies every signature recovers
  to the selected schedule witness and enforces strict `> 2/3` signed weight,
  and keeps missing seal material visible as a rollout blocker. Bounded
  `--solid-block-ancestor-depth` and
  `--solid-block-confirmation-depth` reads now collect the non-placeholder
  signed header chains, verify ancestor/confirmation linkage, active schedule
  membership, monotonic timestamps, and strict `> 2/3` unique confirmation
  weight, and keep missing or insufficient header evidence visible as rollout
  blockers. Post-submit live JSON now also exposes a fail-closed
  `source_event_transaction_production_ready` flag plus blockers, and marks it
  true only when transaction source proof, solid-block header proof, expected
  witness-schedule hash, witness seal, signed ancestors, and signed
  confirmations are all present and verified; source-record preflight now feeds
  `--source-trust-anchor-hash` into that expected witness-schedule pin and
  rejects drift from a separately supplied schedule hash. When the active
  schedule differs from that trust-anchor hash, repeated
  `--witness-schedule-transition-json` inputs now let live evidence prove the
  canonical parent-signed transition chain from trust anchor to active schedule
  before setting the production-ready bit. Python, JavaScript, Kotlin/JVM, Java
  Android, and Swift SDK/operator surfaces now expose canonical TRON
  solid-block, witness-seal, and witness-schedule transition message/seal bytes
  and hashes, including fail-closed checks that bind the next schedule
  payload/hash, transition message hash, parent schedule hash, and parent
  witness signatures to selected parent-schedule witnesses before seal hashing.
  The helper now also
  recomputes the Rust TRON template component hashes and rejects template
  source trust-anchor, consensus-verifier, message-inclusion, or
  finality-policy hashes before governance TOML is rendered.
  The TRON Groth16/bn254 wrapper and relayer package path are wired, and
  configured material/deployment admission now rejects even deployment-tagged
  legacy receipt-MPT-only proofs that lack the governed transaction-source
  call. Remaining TRON production work is verifier deployment, collection of
  actual live mainnet evidence with deployed contract addresses, and live
  rollout of the governed source bridge transaction-call path. Live
  route-canary evidence capture also binds the visible
  `wallet/gettransactionbyid` owner to the owner encoded in the hashed
  `raw_data_hex`, so full-rollout evidence cannot mix a valid transaction body
  with drifted JSON metadata, and direct/all-lanes TOML now requires that
  binding as first-class route-canary metadata. The legacy `TransactionInfo`
  receipt-log/MPT path
  remains structural only. TVM
  account/storage proofs remain out of scope
  until TRON exposes a
  consensus-authenticated contract-state root; future state-derived claims need
  a new source proof plan and material prefix instead of reusing
  `TronDposReceiptProof`. Land the production
  Solana, TON, and TRON prover/verifier integrations behind the SDK proof request APIs, deploy
immutable destination verifiers plus TON/TRON verifier-contract bindings,
produce multi-lane integration evidence, publish operator runbooks, and
incorporate testnet-driven feedback
from wallet and service integrations.

## IVM, Kotodama, and Norito

**Status:** active first-release hardening.

- Keep the Iroha Virtual Machine syscall and pointer-ABI surface deterministic
  across hardware and peers.
- Make `iroha contract dev` the default first-release contract workflow,
  including manifest-sourced builds, generated interfaces, schema docs,
  profile-aware doctor/smoke commands, and Kotodama test/debug loops.
- Static compiler-derived access descriptors now cover the formerly opaque
  peer, subscription, VRF epoch seed, AXT, and Soracloud host helper syscalls;
  dynamic, malformed, and test-mode-only helper payloads intentionally remain
  fail-closed instead of reintroducing wildcard production manifests.
- Preserve canonical Norito headers and wire layouts for blocks, transactions,
  SDK fixtures, and cross-library compatibility tests. The JavaScript pure
  Norito fallback now covers asset-definition registration frames, and
  Java/Kotlin columnar helpers cover optional string/u32 plus bytes+bool row
  shapes, so remaining SDK parity work should focus on new observable wire
  formats as they land.

**Next checkpoints:** ABI golden updates when the syscall surface changes,
expanded cross-SDK vector coverage, and updated docs for any observable layout
or ABI behavior.

## Privacy, ZK, and FHE

**Status:** active research-to-product integration.

- Replace current deterministic BFV-shaped evaluation scaffolding with the full
  BFV-RNS implementation planned for release.
- Broaden cross-SDK deterministic vectors for encrypted payloads, receipts, and
  opening verification.
- Keep Soracloud FHE multi-input behavior covered at the source level while the
  BFV-RNS implementation is still pending. The current Rust corridor covers
  deterministic Add/Multiply folds, malformed late-operand rejection,
  multi-input admission/output projection, output commitment order binding, and
  shared Add/Multiply/RotateLeft/Bootstrap operation-output vectors with
  pinned public-key and evaluation-key bundle metadata, per-entry
  relinearization component digests, rotation/bootstrap refresh `c0`/`c1`
  component digests, and adversarial refresh-material rejection for the public
  rotation/bootstrap keys.
- Keep BFV encrypted-input SDK vectors shared instead of local-only. The current
  fixture set covers the baseline identifier envelope plus Soracloud three-input
  Add and Multiply operand envelopes in JavaScript, Swift, Kotlin/JVM, and Java
  Android test surfaces. Those SDK lanes now also validate the shared
  operation fixture's component-level evaluation-key metadata so missing,
  zeroed, duplicate, or count-drifted key-component vectors are caught outside
  Rust, while the Rust executor consumes the same fixture for operation-output
  digests and plaintext-slot checks.
- Keep Soracloud FHE governance parameter fixtures runtime-bound instead of
  descriptor-only. The canonical parameter-set, execution-policy, governance
  bundle, and job-spec fixtures now target the registered `bfv-default`
  RAM-LFE BFV profile; core admission consumes the shared bundle and rejects
  backend, polynomial-degree, slot-count, plaintext-width, ciphertext-chain, and
  parameter digest drift. Execution policies now also pin the
  domain-separated BFV evaluation-key bundle digest, and `RunSoracloudFheJob`
  rejects structurally valid but ungoverned key bundles before output state is
  emitted.
- Keep signed and proof-attestation identifier receipt compatibility
  fixture-backed instead of local-only. The current shared fixture pins
  canonical payload bytes, Iroha prehash, resolver signature, signed/proof
  attestation bytes, and adversarial receipt/policy mutations across
  the Rust data model, Torii runtime claim-receipt signing path, JavaScript,
  Swift, Kotlin/JVM, and Java Android.
- Keep ZK-ACE authorization SDK surfaces aligned with executable chain
  support. Python now exposes identity commitment lifecycle instructions,
  authorized-transfer submission, and fail-closed capability metadata so BOI
  Privacy Lab and other catalog consumers do not advertise ZK-ACE execution
  when the native instruction surface is stale. JavaScript prepared-proof
  builders now also enforce ZK-ACE public-input version `1` and reject
  authorization proofs whose public inputs do not match the requested
  transparent transfer fields before an instruction is emitted. Core
  chain-admission tests now cover rotated and revoked identity commitments,
  unsupported action classes, transaction digest/account substitution, and
  mutated ZK-ACE/STARK public inputs.
- Fold focused ZK/FHE adversarial tests into the long workspace validation
  corridor.

**Next checkpoints:** extend the component-level BFV key-bundle vectors into
full BFV-RNS modulus-chain, packed Galois-switching, and bootstrapping key
vectors, and fold the focused ZK/FHE fixture corridor into broader release
validation.

## Consensus, Performance, and Operations

**Status:** active optimization.

- Wire the canonical Sumeragi V1 pure engine through the live network,
  validation, payload, telemetry, and storage adapters while preserving
  deterministic consensus behavior and the hard consensus cadence gates.
- Keep permissioned and NPoS execution on one state machine; validator-set
  source and strict quorum math are the only mode differences.
- Keep the Sumeragi formal coverage guard in CI so runner modes, CI commands,
  README commands, and referenced TLA+/CFG files stay synchronized as new gates
  land.
- Use measured matrix runs, not speculative settings, before accepting higher
  throughput targets.
- Keep hardware acceleration paths feature-gated with deterministic scalar
  fallbacks.

**Next checkpoints:** Sumeragi V1 adapter integration, certified-block
recovery soak coverage, peer-gap and DA/RBC tail-latency reductions under the
broadened rotating-fault evidence, broader formal coverage beyond the current
commit-path, frontier, TLC-cross-checked fork-safety, TLC-cross-checked
quorum-policy, TLC-cross-checked RBC deliver-quorum,
TLC-cross-checked RBC causality gate, TLC-cross-checked RBC DELIVER acceptance gate,
TLC-cross-checked RBC commit-processing gate,
TLC-cross-checked RBC local READY emission gate (`rbc-ready-emission`),
TLC-cross-checked RBC local DELIVER emission gate (`rbc-deliver-emission`),
TLC-cross-checked RBC delivered-session rebroadcast gate (`rbc-delivered-rebroadcast`),
TLC-cross-checked RBC stalled-rebroadcast cursor gate (`rbc-rebroadcast-cursor`),
TLC-cross-checked RBC stalled-rebroadcast action gate (`rbc-rebroadcast-action`),
TLC-cross-checked RBC next-due scheduler gate (`rbc-next-due`),
TLC-cross-checked RBC chunk target helper gate, TLC-cross-checked RBC chunk payload-cap helper gate
(`rbc-chunk-payload-cap`), TLC-cross-checked RBC rebroadcaster selection helper gate,
TLC-cross-checked RBC weighted chunk allocation helper gate, TLC-cross-checked RBC payload chunking helper gate,
TLC-cross-checked RBC payload layout helper gate (`rbc-payload-layout`),
TLC-cross-checked RBC session chunk-ingest helper gate
(`rbc-session-chunk-ingest`), TLC-cross-checked RBC READY/DELIVER session
recording helper gate (`rbc-session-ready-deliver`), TLC-cross-checked RBC
delivered-payload byte telemetry helper gate (`rbc-delivered-payload-bytes`),
TLC-cross-checked RBC RS16 initial fanout helper
gate, TLC-cross-checked RBC chunk broadcast order helper gate,
TLC-cross-checked pending-RBC stash gate, TLC-cross-checked pending-RBC status snapshot helper gate
(`pending-rbc-status`), TLC-cross-checked ingress dedup cache helper gate
(`ingress-dedup-cache`), TLC-cross-checked inbound consensus status counter helper gate
(`ingress-status-counters`), TLC-cross-checked consensus message
kind/outcome/reason label helper gate (`consensus-message-labels`),
TLC-cross-checked phase-latency status projection helper gate
(`phase-latency-status`), TLC-cross-checked telemetry availability/QC/RBC/pipeline status
projection helper gate (`telemetry-status`), TLC-cross-checked lane-detail status stripping and
projection helper gate (`lane-detail-status`), TLC-cross-checked DvP/PvP settlement telemetry
status helper gate (`settlement-status`), TLC-cross-checked Nexus fee/staking economics status
helper gate (`nexus-economics-status`), TLC-cross-checked NPoS repair fanout coverage status
helper gate (`npos-repair-coverage-status`), TLC-cross-checked mode/PRF/mode-flip status
projection helper gate (`mode-status`), TLC-cross-checked consensus
capability status projection helper gate (`consensus-caps-status`),
TLC-cross-checked effective timing status projection
helper gate (`effective-timing-status`), TLC-cross-checked transaction queue backpressure status
projection helper gate (`tx-queue-backpressure-status`), status history
projection helper gate (`history-status`), commit-quorum status projection
helper gate (`commit-quorum-status`), commit-inflight status projection helper gate
(`commit-inflight-status`), TLC-cross-checked RBC status lookup helper gate,
TLC-cross-checked RBC status retention/update-pruning helper gate (`rbc-status-retention`),
TLC-cross-checked RBC status persistence/fallback helper gate (`rbc-status-persistence`),
TLC-cross-checked RBC status handle lifecycle helper gate
(`rbc-status-handle`), TLC-cross-checked RBC backlog/status snapshot
helper gate (`rbc-backlog-status`), RBC abort status counter/latest-slot helper
gate (`rbc-abort-status`), RBC mismatch status counter/label helper gate
(`rbc-mismatch-status`), RBC progress-stage synchronization helper
gate (`rbc-progress-stage`), RBC hot-repair/backpressure helper gate
(`rbc-hot-repair`), RBC repair request helper gate (`rbc-repair-request`),
RBC targeted READY/DELIVER repair helper gate (`rbc-targeted-repair`),
RBC outbound chunk flush helper gate (`rbc-outbound-flush`),
RBC chunk post scheduling/debug-mask helper gate (`rbc-chunk-post-debug`),
RBC READY/DELIVER deferral throttle helper gate (`rbc-deferral-throttle`),
RBC missing-INIT broad rebroadcast gate (`rbc-missing-init-rebroadcast`),
RBC persisted chunk sampling helper gate
(`rbc-sampling`), RBC persisted session-store guard gate
(`rbc-store`), RBC store status accounting helper gate (`rbc-store-status`),
RBC store pressure log throttling helper gate (`rbc-store-pressure-log`),
round-gap marker/snapshot/EMA status helper gate (`round-gap-status`),
RBC recovery helper gate, RBC missing BlockCreated recovery
helper gate (`rbc-missing-block-recovery`), RBC unverified-roster escape-hatch gate,
RBC signing-preimage gate, classic Vote/VRF
signing-preimage gate, classic Vote/QC signature-verification gate,
invalid-signature kind/outcome label helper gate
(`invalid-signature-labels`),
invalid-signature throttle/penalty helper gate (`invalid-signature-throttle`),
TLC-cross-checked vote-validation drop telemetry status helper gate
(`vote-validation-drop-status`),
penalty offender/epoch/roster selection helper gate
(`penalty-offender-selection`),
consensus penalty action derivation/application helper gate
(`consensus-penalty-action`),
penalty status projection helper gate (`penalty-status`),
local peer removed flag helper gate (`local-peer-removed-status`),
TLC-cross-checked execution-witness recorder lifecycle/keying helper gate
(`exec-witness-recorder`),
TLC-cross-checked execution-witness access-key parser helper gate
(`exec-witness-access-key`),
execution-witness root projection helper gate (`exec-witness-roots`),
TLC-cross-checked sparse-Merkle path/hash helper gate (`smt-path-hash`),
RBC compact block-message helper gate (`block-message-rbc-compact`),
consensus block-message priority helper gate (`block-message-priority`),
consensus block-message height/view projection helper gate
(`block-message-height-view`),
consensus block-message log/status kind projection helper gate
(`block-message-kind`),
TLC-cross-checked Kura replica advert ingress helper gate
(`kura-replica-advert`),
consensus message timing/control/native-AMX projection helper gate
(`message-projection`),
pipeline event emission helper gate (`pipeline-event-emission`),
cached block-message Norito frame helper gate (`block-message-wire`),
BlockCreated frontier metadata wire/rebuild helper gate
(`block-created-frontier-wire`),
TLC-cross-checked canonical block payload bytes helper gate
(`block-payload-canonicalization`),
cached proposal rebroadcast helper gate (`cached-proposal-rebroadcast`),
TLC-cross-checked frontier block-sync hint/direct-response permit gate
(`frontier-block-sync-hint`),
exact-slot frontier recovery activity helper gate
(`frontier-same-slot-activity`),
frontier reassembly activity helper gate
(`frontier-reassembly-activity`),
frontier quorum-timeout actionable-owner cleanup helper gate
(`frontier-quorum-owner-actionable`),
contiguous-frontier sidecar retarget helper gate
(`frontier-sidecar-retarget`),
contiguous-frontier sidecar expected-hash helper gate
(`frontier-sidecar-expected-hash`),
contiguous-frontier payload-hint selector helper gate
(`contiguous-frontier-payload-hint`),
contiguous-frontier parent QC-hint retarget helper gate
(`frontier-parent-qc-hint-retarget`),
TLC-cross-checked live-frontier idle missing-QC suppression helper gate
(`live-frontier-idle-missing-qc`),
TLC-cross-checked missing-QC reacquire admission helper gate
(`missing-qc-reacquire-admission`),
TLC-cross-checked missing-QC reacquire action orchestration helper gate
(`missing-qc-reacquire-action`),
TLC-cross-checked missing commit-QC actionable dependency helper gate
(`missing-commit-qc-actionable`),
TLC-cross-checked same-height missing-QC stall dampening helper gate
(`missing-qc-height-stall`),
TLC-cross-checked same-height missing-QC stall range-pull helper gate
(`missing-qc-stall-range-pull`),
TLC-cross-checked same-height missing-payload fetch-window and hash-miss cap helper gate
(`missing-payload-fetch-window`),
TLC-cross-checked canonical contiguous-frontier reanchor helper gate
(`canonical-frontier-reanchor`),
TLC-cross-checked contiguous-frontier repair view-change suppression helper gate
(`frontier-repair-view-change`),
TLC-cross-checked contiguous-frontier recovery advance state-machine helper gate
(`frontier-recovery-advance`),
TLC-cross-checked same-height no-proposal storm recovery helper gate
(`same-height-no-proposal-storm`),
TLC-cross-checked VRF commit/reveal admission gate, TLC-cross-checked VRF
epoch-window arithmetic helper gate
(`vrf-epoch-window`), TLC-cross-checked VRF epoch-boundary finalization helper gate
(`vrf-epoch-boundary`), TLC-cross-checked VRF epoch restore/snapshot/observation-merge helper gate
(`vrf-epoch-restore`), TLC-cross-checked local VRF material derivation helper gate
(`vrf-material-derivation`), TLC-cross-checked local VRF emission state helper gate
(`vrf-local-state`), TLC-cross-checked VRF penalties report store helper gate
(`vrf-penalties-report`), TLC-cross-checked classic inbound vote-admission gate, vote
TLC-cross-checked duplicate-key helper gate (`vote-duplicate-key`),
TLC-cross-checked evidence freshness horizon helper gate,
TLC-cross-checked evidence canonicalization/deduplication helper
gate (`evidence-canonicalization`), TLC-cross-checked evidence validation helper gate
(`evidence-validation`), TLC-cross-checked double-vote detection/recording helper gate
(`double-vote-recording`), TLC-cross-checked invalid-QC shape helper gate
(`invalid-qc-shape`), TLC-cross-checked QC validation evidence helper gate
(`qc-validation-evidence`), TLC-cross-checked QC validation reason/evidence label helper gate
(`qc-validation-reason`), TLC-cross-checked block-sync QC retry/fallback helper gate
(`block-sync-qc-fallback`), TLC-cross-checked block-sync QC status helper gate
(`block-sync-qc-status`), TLC-cross-checked block-sync locked-QC helper gate
(`block-sync-locked-qc`), TLC-cross-checked known-block QC work enqueue gate
(`known-block-qc-enqueue`), TLC-cross-checked known-block QC work preparation gate
(`known-block-qc-work`), TLC-cross-checked known-block QC work queue drain gate
(`known-block-qc-drain`), TLC-cross-checked committed signed-quorum fetch fallback gate
(`signed-quorum-fetch-fallback`), TLC-cross-checked commit-QC-only fetch response
dispatch gate (`commit-qc-only-fetch-response`), TLC-cross-checked
BlockSyncUpdate gossip target-selection helper gate (`block-sync-update-targets`),
TLC-cross-checked cached BlockSyncUpdate proof/vote attachment helper gate
(`apply-cached-qcs`), TLC-cross-checked uncertified block-sync roster
admission gate (`block-sync-roster`), TLC-cross-checked block-sync roster
source/drop status helper gate (`block-sync-roster-status`), TLC-cross-checked
BlockSyncUpdate embedded-vote filtering and deferral handoff gate
(`block-sync-vote-deferral`), TLC-cross-checked already-known hintless
BlockSyncUpdate fast-path gate (`block-sync-known-hintless`),
TLC-cross-checked DA implicit BlockSyncUpdate recovery gate
(`block-sync-implicit-recovery`),
TLC-cross-checked frontier vote-placeholder gate (`block-sync-vote-placeholder`),
TLC-cross-checked known-block snapshot-hint gate (`block-sync-snapshot-hint`),
TLC-cross-checked known-block snapshot-roster gate (`block-sync-snapshot-roster`),
TLC-cross-checked no-verifiable-roster BlockSyncUpdate gate
(`block-sync-no-roster`),
TLC-cross-checked selected-roster known-block terminal replay gate
(`block-sync-known-roster`),
TLC-cross-checked selected-roster known-block BlockSyncUpdate gate
(`block-sync-known-selected-roster`),
TLC-cross-checked selected-roster BlockSyncUpdate signature gate
(`block-sync-selected-signatures`),
TLC-cross-checked selected-roster BlockSyncUpdate QC candidate/evidence gate
(`block-sync-selected-qc`),
TLC-cross-checked selected-roster BlockSyncUpdate quorum/missing-QC repair gate
(`block-sync-selected-quorum`),
TLC-cross-checked stale BlockCreated/recovery-mode helper gate
(`block-sync-recovery-mode`),
TLC-cross-checked selected-roster BlockSyncUpdate apply/recovery-mode gate
(`block-sync-selected-apply`),
TLC-cross-checked selected-roster BlockSyncUpdate post-apply QC prefilter gate
(`block-sync-selected-qc-prefilter`),
TLC-cross-checked selected-roster BlockSyncUpdate post-prefilter QC process gate
(`block-sync-selected-qc-process`),
TLC-cross-checked selected-roster BlockSyncUpdate unknown-block QC cache gate
(`block-sync-selected-qc-cache`),
TLC-cross-checked BlockSyncUpdate stale-view admission gate
(`block-sync-stale-view`),
TLC-cross-checked committed-height BlockSyncUpdate conflict/evidence gate
(`block-sync-commit-conflict`),
TLC-cross-checked block-sync warning throttle helper gate
(`block-sync-warning-throttle`),
TLC-cross-checked QC-insufficient warning throttle helper gate
(`qc-insufficient-warning`),
TLC-cross-checked canonical committed fetch/body response deferral gate
(`fetch-response-deferral`),
TLC-cross-checked exact body fetch handler gate
(`fetch-block-body-handle`),
TLC-cross-checked background consensus frame-cap preparation gate
(`background-frame-cap`),
TLC-cross-checked background request dispatch fallback gate
(`background-dispatch`),
TLC-cross-checked background scheduler bypass gate
(`background-bypass`),
TLC-cross-checked background fallback network dispatch gate
(`background-fallback`),
TLC-cross-checked fetch-pending response send gate
(`fetch-pending-response-send`),
TLC-cross-checked fetch-pending batch response fanout gate
(`fetch-pending-responses-batch`),
TLC-cross-checked pending fetch/body readiness flush gate
(`pending-response-flush`),
TLC-cross-checked deferred BlockSyncUpdate helper gate
(`deferred-block-sync-helper`),
TLC-cross-checked deferred BlockSyncUpdate cache/defer integration gate
(`deferred-block-sync-cache`),
TLC-cross-checked deferred BlockSyncUpdate replay gate
(`deferred-block-sync-replay`),
TLC-cross-checked future BlockSyncUpdate drop/window gate
(`block-sync-future-window`),
TLC-cross-checked RBC block-body repair admission gate
(`block-body-repair`),
TLC-cross-checked exact body requester stash-window gate
(`block-body-request-stash`),
TLC-cross-checked same-height block-body repair admission gate
(`same-height-block-body-repair`),
TLC-cross-checked block-body repair observed epoch source gate
(`block-body-repair-epoch`),
TLC-cross-checked direct commit-QC source selection gate
(`direct-commit-qc-for-block`),
TLC-cross-checked QC materialization/Kura recovery gate
(`materialize-qc`),
TLC-cross-checked BlockBodyResponse direct commit-QC extraction gate
(`block-body-direct-commit-qc`),
TLC-cross-checked detached BlockBodyResponse commit-QC handling gate
(`block-body-detached-commit-qc`),
TLC-cross-checked exact BlockBodyResponse fallback/companion dispatch gate
(`block-body-response-dispatch`),
TLC-cross-checked invalid-proposal evidence builder helper gate
(`invalid-proposal-evidence`),
TLC-cross-checked proposal mismatch helper gate (`proposal-mismatch`),
TLC-cross-checked proposal cache helper gate (`proposal-cache`),
TLC-cross-checked proposal-hint admission gate (`proposal-hint`),
TLC-cross-checked stale proposal-hint repair gate
(`stale-proposal-hint-repair`), TLC-cross-checked stale RBC hint repair gate
(`stale-rbc-hint-repair`),
TLC-cross-checked proposal metadata admission gate (`proposal-admission`),
TLC-cross-checked peer-admin detection helper gate
(`peer-admin-detection`), TLC-cross-checked QC signer-bitmap admission
(`qc-signers`), raw QC signer-count helper gate
(`qc-signer-count`), TLC-cross-checked direct BlockCreated admission gate
(`block-created-admission`), TLC-cross-checked missing-block request clear
helper gate (`missing-request-clear`), TLC-cross-checked missing-block clear
reason helper gate
(`missing-block-clear`), TLC-cross-checked proposal budget/cap helper gate
(`proposal-budget`), TLC-cross-checked non-RBC payload frame budget helper gate
(`non-rbc-payload-budget`), TLC-cross-checked proposal backpressure
classification helper gate (`proposal-backpressure`), TLC-cross-checked
proposal-defer warning throttle helper gate (`proposal-defer-warning`),
TLC-cross-checked proposal batch trim/canonicalization helper gate
(`proposal-batch`), TLC-cross-checked lane/dataspace commitment snapshot
builder gate (`commitment-snapshot-builder`),
TLC-cross-checked collector retry/gossip helper gate (`collector-plan`),
TLC-cross-checked lane interleave routing-decision helper gate
(`lane-interleave`), TLC-cross-checked collector fanout/selection helper gate
(`collector-selection`), TLC-cross-checked topology ordered-roster mutation
helper gate (`topology-mutation`), TLC-cross-checked PRF leader/shuffle
topology helper gate (`prf-leader-shuffle`), TLC-cross-checked topology
fanout/redundant-send helper gate, TLC-cross-checked active topology selection
helper gate
(`active-topology-selection`), TLC-cross-checked trusted-peer P2P topology
refresh helper gate
(`p2p-topology-trusted`), TLC-cross-checked P2P topology refresh coordinator
gate
(`p2p-topology-refresh`), TLC-cross-checked quorum retransmit target helper
gate,
TLC-cross-checked retransmit backpressure pacing helper gate, paced retransmit
target selection helper gate (`paced-retransmit-targets`) with TLC
cross-checks, TLC-cross-checked quorum reschedule backoff helper
gate, TLC-cross-checked DA/RBC availability reschedule gate
(`rbc-availability-reschedule`),
TLC-cross-checked vote-backed reassembly stall helper gate
(`vote-backed-reassembly-stall`),
TLC-cross-checked completed quorum view-advance helper gate
(`completed-quorum-view-advance`),
TLC-cross-checked quorum rebroadcast dispatch helper gate
(`quorum-rebroadcast-dispatch`),
TLC-cross-checked isolated vote-backed frontier handoff helper gate
(`isolated-vote-backed-handoff`),
TLC-cross-checked pre-timeout vote-backed frontier retransmit handoff gate
(`preemptive-vote-backed-retransmit`),
TLC-cross-checked near-quorum preemptive missing-payload escalation coordinator gate
(`near-quorum-preemptive-escalation`),
TLC-cross-checked manifest-gated quorum reschedule helper gate, raw QC signer-count helper
gate (`qc-signer-count`), TLC-cross-checked signer-bitmap construction helper gate
(`build-signers-bitmap`), canonical/view signer-index
normalization helper gate (`signer-index-normalization`), TLC-cross-checked
commit-root consistency, TLC-cross-checked commit-pipeline recovery gate,
TLC-cross-checked known-block commit-QC recovery
helper gate, TLC-cross-checked stale-view commit-QC fetch admission helper gate
(`stale-view-commit-qc-fetch`), TLC-cross-checked commit-anchor QC promotion
helper gate (`commit-anchor-qc`),
TLC-cross-checked committed-height QC admission helper gate (`committed-height-qc`),
TLC-cross-checked empty-block QC drop helper gate (`empty-block-qc-drop`),
TLC-cross-checked pending-progress accounting helper gate,
TLC-cross-checked pending-block lifecycle helper gate,
TLC-cross-checked pending-block marker/cooldown helper gate,
TLC-cross-checked pending-block Kura retry helper gate (`kura-retry`),
TLC-cross-checked commit-pipeline scheduling gate,
TLC-cross-checked precommit vote-count helper gate (`precommit-vote-count`),
TLC-cross-checked precommit vote lock filter gate
(`drop-precommit-vote-for-lock`),
TLC-cross-checked set-based voting signer-count helper gate
(`voting-signer-count`),
TLC-cross-checked cached vote-log epoch replay helper gate
(`distinct-vote-epochs`),
TLC-cross-checked NEW_VIEW highest-QC vote-selection helper gate
(`new-view-highest-qc-votes`),
TLC-cross-checked frontier NEW_VIEW catch-up helper gate
(`frontier-new-view-catch-up`),
TLC-cross-checked late NEW_VIEW near-quorum emission helper gate
(`late-new-view-emission`),
TLC-cross-checked near-quorum NEW_VIEW rebroadcast helper gate
(`near-quorum-new-view-rebroadcast`), TLC-cross-checked precommit-QC
locked-chain wrapper gate
(`precommit-qc-extends-locked`),
TLC-cross-checked requester roster-proof detection helper gate
(`requester-roster-proof`),
TLC-cross-checked online-validator and relay counter helper gate
(`online-validator-relay-counters`),
TLC-cross-checked commit-result drain gate (`commit-result-drain`),
TLC-cross-checked commit-drain summary aggregation helper gate
(`commit-drain-summary`),
TLC-cross-checked commit-pipeline timing sample helper gate
(`commit-pipeline-sample`),
TLC-cross-checked commit-pipeline status recorder helper gate
(`commit-pipeline-status`),
TLC-cross-checked autoscale transition commit gate
(`autoscale-transition`),
TLC-cross-checked commit-QC signer quorum helper gate
(`commit-quorum-signers`),
TLC-cross-checked signature-index recovery helper gate
(`signature-index-recovery`),
TLC-cross-checked commit-QC cache/history lookup helper gate
(`commit-qc-lookup`),
TLC-cross-checked embedded-QC roster bootstrap helper gate
(`embedded-qc-roster`),
TLC-cross-checked cached-QC precommit signer record helper gate
(`precommit-signer-record`),
TLC-cross-checked roster-validation memo cache helper gate
(`roster-validation-memo`),
TLC-cross-checked roster-validation cached wrapper helper gate
(`roster-validation-cached`), TLC-cross-checked core roster-validation helper
gate (`roster-validation-core`), TLC-cross-checked roster artifact selection
helper gate (`roster-artifact-selection`), TLC-cross-checked block roster cache
helper gate (`block-roster-caches`), TLC-cross-checked block-sync roster
evidence helper gate (`block-sync-roster-evidence`), TLC-cross-checked
block-sync history roster helper gate (`block-sync-history-roster`),
TLC-cross-checked persisted block-sync roster selection helper gate
(`persisted-roster-selection`), TLC-cross-checked BlockSyncUpdate roster
hydration helper gate (`block-sync-update-roster`), TLC-cross-checked roster
index projection helper gate (`roster-index-projection`), TLC-cross-checked
membership-view hash helper gate (`membership-view-hash`), TLC-cross-checked
membership mismatch status helper gate (`membership-mismatch-status`),
TLC-cross-checked membership advert publication helper gate
(`membership-advert`), TLC-cross-checked membership mismatch
ingress/fail-closed helper gate (`membership-mismatch-ingress`),
TLC-cross-checked consensus-params ingress helper gate
(`consensus-params-ingress`), TLC-cross-checked prevalidated commit artifact
trust helper gate (`prevalidated-commit-artifact`), TLC-cross-checked
commit-job dispatch gate,
commit-worker channel capacity helper gate (`commit-worker-config`), slow
commit-stage timing threshold helper gate (`commit-stage-timing-threshold`),
commit-inflight timeout gate, post-commit pacemaker kick gate, idle-view
proposal budget gate,
TLC-cross-checked pacemaker core state-machine helper gate
(`pacemaker-core`), TLC-cross-checked pacemaker evaluation gate,
TLC-cross-checked pacing governor helper gate,
cached proposal-slot timeout gate,
pending fast-path timeout helper gate (`pending-fast-path-timeout`),
stalled pending-block timeout decision gate (`stalled-pending-timeout`),
stalled pending-frontier timeout helper gate (`stalled-pending-frontier-timeout`),
missing-QC timing helper gate,
idle backlog signal helper gate (`idle-backlog-signals`),
proposal-liveness state helper gate (`proposal-liveness`),
exact-frontier slot tracker FSM gate (`frontier-slot-tracker`),
exact-frontier slot helper gate (`frontier-slot-helpers`),
exact-frontier proposal grace helper gate (`frontier-proposal-grace`),
slot tracker state helper gate (`slot-tracker-state`),
timeout/cooldown derivation helper gate (`timeout-derivation`),
round/view helper gate (`round-view-helpers`),
PhaseTracker mutable state helper gate (`phase-tracker`),
TLC-cross-checked round-trace status recorder gate (`round-trace-status`),
failed-commit/block-sync helper gate (`failure-recovery-helpers`),
TLC-cross-checked transaction requeue branch helper gate
(`requeue-transactions`),
TLC-cross-checked tick/deadline scheduling helper gate, worker tick-gap helper
gate (`worker-tick-gap`),
TLC-cross-checked proposal parent resolution gate,
TLC-cross-checked highest-QC dependency deferral gate,
TLC-cross-checked precommit-QC view-change selector gate,
TLC-cross-checked commit-evidence replay gate, TLC-cross-checked block-sync recovery gate, TLC-cross-checked direct certified-block fetch gate,
TLC-cross-checked missing-block ingress fetch gate, TLC-cross-checked payload progress availability gate, TLC-cross-checked highest-QC fetch body-known gate, TLC-cross-checked local payload availability gate, TLC-cross-checked local block-known routing gate, TLC-cross-checked lock-safety block-known routing gate, TLC-cross-checked missing locked-QC payload recovery gate (`missing-locked-qc-recovery`), TLC-cross-checked local signed-block materialization gate, TLC-cross-checked authoritative payload progress gate, TLC-cross-checked hash-level authoritative block payload gate, TLC-cross-checked pending-block active-for-tip gate, TLC-cross-checked pending fast-unblock decision gate, TLC-cross-checked blocking pending-block counter gate, TLC-cross-checked quorum recovery vote-drain urgency gate, TLC-cross-checked frontier body-gap payload-drain urgency gate, TLC-cross-checked RBC authoritative payload progress gate, TLC-cross-checked slot authoritative payload gate, TLC-cross-checked missing-block fetch planner, TLC-cross-checked recovery status counter helper gate (`recovery-status-counters`), TLC-cross-checked QC rebuild status counter helper gate (`qc-rebuild-status`), TLC-cross-checked QC rebuild quorum reachability helper gate (`qc-rebuild-quorum`), TLC-cross-checked collector-targeting status counter helper gate (`collector-targeting-status`), TLC-cross-checked deferred recovery status counter helper gate (`deferred-recovery-status`), TLC-cross-checked missing-QC liveness status counter helper gate (`missing-qc-liveness-status`), TLC-cross-checked sidecar/no-proposal status counter helper gate (`sidecar-no-proposal-status`), TLC-cross-checked deterministic committee status helper gate (`deterministic-committee-status`), TLC-cross-checked timing/liveness status counter helper gate (`timing-status-counters`), TLC-cross-checked roster-recovery status counter helper gate (`roster-recovery-status`), TLC-cross-checked range-pull recovery helper gate (`range-pull-recovery`), TLC-cross-checked range-pull status counter helper gate (`range-pull-status`), TLC-cross-checked round-recovery bundle window helper gate (`round-recovery-bundle-window`),
TLC-cross-checked recovery-FSM reason classifier/rank/sort helper gate (`recovery-fsm-reason`),
TLC-cross-checked committed-edge conflict suppression gate,
TLC-cross-checked lock-rejected branch sink gate, TLC-cross-checked active-height lock-reject recovery gate,
TLC-cross-checked missing-block hard-cap recovery gate,
TLC-cross-checked missing-block hard-cap cleanup gate,
TLC-cross-checked missing-block view-change escalation gate, TLC-cross-checked precommit vote-emission gate,
TLC-cross-checked native AMX attestation gate,
TLC-cross-checked native AMX queue-journal replay gate, TLC-cross-checked native AMX routing-plan projection gate,
TLC-cross-checked native AMX receipt validation gate, TLC-cross-checked native AMX control-plane ingress gate,
TLC-cross-checked vNext chain-order helper gate, TLC-cross-checked vNext stake-weight/quorum helper gate
(`vnext-stake-weight`), TLC-cross-checked vNext re-chain helper gate,
TLC-cross-checked vNext re-chain error label helper gate, TLC-cross-checked
vNext aggregate certificate verification gate, TLC-cross-checked vNext
signing-preimage gate, TLC-cross-checked vNext control-certificate ingress gate,
TLC-cross-checked vNext slot-lifecycle gate, TLC-cross-checked vNext validation
ownership gate, TLC-cross-checked vNext deadline/protection helper gate
(`vnext-deadline-protection`), vNext performance-fault config conversion gate
(`vnext-performance-config`), pending-block validation worker config helper gate
(`validation-worker-config`), TLC-cross-checked validation stall/redrive helper gate
(`validation-stall-redrive`), validation redrive reason label/distinctness
helper gate (`validation-redrive-label`), validation ownership cleanup helper gate
(`validation-ownership-cleanup`), TLC-cross-checked vote/QC verification cache-key identity helper
gate (`verify-cache-key`), TLC-cross-checked async vote-verification ownership gate,
vote-signature verification worker config helper gate
(`vote-verify-worker-config`), TLC-cross-checked async QC aggregate-verification ownership gate,
QC aggregate-verification worker config helper gate (`qc-verify-worker-config`),
TLC-cross-checked worker-loop drain scheduler gate,
TLC-cross-checked actor-gate priority/fairness gate,
TLC-cross-checked worker-loop budget/adaptive-cap gate,
TLC-cross-checked worker ingress routing gate,
worker-loop stage helper gate, TLC-cross-checked worker-queue status accounting gate,
TLC-cross-checked NPoS VRF epoch-seal staging gate,
commit-anchor QC promotion helper gate (`commit-anchor-qc`),
committed-height QC admission helper gate (`committed-height-qc`),
TLC-cross-checked proposal assembly gate, TLC-cross-checked Kura durability
commit retry gate, TLC-cross-checked Kura persistence status counter/snapshot helper gate
(`kura-store-status`), TLC-cross-checked post-commit cleanup gate, TLC-cross-checked frontier-gap
realignment gate, frontier block-sync hint/direct-response permit gate,
TLC-cross-checked same-height vote conflict helper gate, aggregate same-height vote-lock helper gate,
TLC-cross-checked proposal stale same-height vote helper gate,
TLC-cross-checked same-height vote recovery view-gap helper gate,
TLC-cross-checked tip-extension helper gate,
TLC-cross-checked DA gate helper gate,
TLC-cross-checked DA gate status counter/snapshot helper gate
(`da-gate-status`),
TLC-cross-checked DA manifest guard helper gate,
TLC-cross-checked consensus handshake capability construction helper gate,
TLC-cross-checked consensus handshake helper gate,
TLC-cross-checked runtime mode flip helper gate,
TLC-cross-checked effective consensus-mode selection helper gate,
TLC-cross-checked effective consensus timing aggregation helper gate,
TLC-cross-checked NEW_VIEW stats helper gate,
TLC-cross-checked NEW_VIEW tracker quorum/selection helper gate (`new-view-tracker`),
TLC-cross-checked timing monitor helper gate,
TLC-cross-checked hotspot summary accumulator helper gate (`hotspot-log-summary`),
TLC-cross-checked adaptive observability timing/fanout helper gate (`adaptive-observability`),
TLC-cross-checked pacing backpressure helper gate,
TLC-cross-checked counter-driven backpressure cooldown helper gate
(`counter-backpressure-cooldown`),
TLC-cross-checked per-reason pacemaker backpressure tracker gate
(`pacemaker-backpressure-tracker`),
TLC-cross-checked locked-QC helper gate,
TLC-cross-checked stake snapshot quorum helper gate,
TLC-cross-checked NPoS validator election helper gate (`validator-election`),
TLC-cross-checked topology role/signature filter gate
(`topology-role-filter`),
TLC-cross-checked live local-vote roster helper gate (`live-vote-roster`),
TLC-cross-checked canonical round-roster helper gate (`canonical-round-roster`),
TLC-cross-checked block-specific vote-roster selection gate (`vote-roster-selection`),
TLC-cross-checked vote-roster cache/support helper gate (`vote-roster-cache`),
TLC-cross-checked commit-topology state/reset helper gate (`commit-topology-state`),
TLC-cross-checked roster index projection helper gate
(`roster-index-projection`),
TLC-cross-checked membership-view hash helper gate (`membership-view-hash`),
TLC-cross-checked membership mismatch status helper gate
(`membership-mismatch-status`),
TLC-cross-checked membership advert publication helper gate
(`membership-advert`),
TLC-cross-checked membership mismatch ingress/fail-closed helper gate
(`membership-mismatch-ingress`),
TLC-cross-checked consensus-params ingress helper gate
(`consensus-params-ingress`),
TLC-cross-checked prevalidated commit artifact trust helper gate
(`prevalidated-commit-artifact`),
TLC-cross-checked commit-job dispatch gate,
TLC-cross-checked precommit signer-history block-sync fallback gate,
TLC-cross-checked pure engine constructor initial-state gate,
TLC-cross-checked pure engine read-only accessor gate,
TLC-cross-checked pure engine tick gate,
TLC-cross-checked pure engine tick unrelated-state preservation gate,
TLC-cross-checked pure engine NewView subject projection helper gate, pure engine certificate
prefilter dispatch gate, pure engine certificate prefilter state-handoff gate,
TLC-cross-checked pure engine certificate prefilter unrelated-state preservation gate,
TLC-cross-checked pure engine view-advance saturation gate,
TLC-cross-checked engine NewView-QC gate,
TLC-cross-checked pure engine exact NewView-QC highest-QC record gate,
TLC-cross-checked pure engine NewView-QC unrelated-state preservation gate,
TLC-cross-checked pure engine exact NewView-QC advance gate,
TLC-cross-checked pure engine handle-dispatch gate,
TLC-cross-checked pure engine top-level argument-forwarding gate,
TLC-cross-checked pure engine top-level output relay gate,
TLC-cross-checked pure engine proposal-ingress gate,
TLC-cross-checked pure engine exact proposal output-field gate,
TLC-cross-checked pure engine exact proposal state-mutation gate,
TLC-cross-checked pure engine proposal unrelated-state preservation gate,
TLC-cross-checked pure engine exact proposal validation-owner gate,
TLC-cross-checked proposal-lock helper gate,
TLC-cross-checked QC-round compatibility helper gate,
TLC-cross-checked QC reference projection helper gate,
TLC-cross-checked QC reference comparator helper gate,
TLC-cross-checked highest-QC record helper gate,
TLC-cross-checked commit-subject helper gate,
TLC-cross-checked payload lookup helper gate,
TLC-cross-checked validation-priority helper gate,
TLC-cross-checked vote-backed evidence helper gate,
TLC-cross-checked vote payload actionable helper gate,
actionable vote-backed proposal evidence helper gate,
slot proposal evidence helper gate,
round liveness helper gate,
roster recovery FSM helper gate,
consensus recovery prune helper gate,
frontier live-owner work helper gate,
keep-frontier-pending-active helper gate,
stale-view pending prune helper gate,
superseded frontier payload retention helper gate
(`superseded-frontier-payload-retention`),
stale missing-block request prune helper gate,
stale missing commit-QC request prune helper gate,
stale RBC session prune helper gate,
highest-QC defer marker prune helper gate,
fast-finality inline validation helper gate,
observer signature-mismatch recovery helper gate (`observer-signature-recovery`),
validation failure finalization helper gate (`validation-failure-finalize`),
validation-reject reason label helper gate
(`validation-reject-reason-label`),
validation-reject status counter/snapshot helper gate
(`validation-reject-status`),
peer-key policy status counter/snapshot helper gate
(`peer-key-policy-status`),
view-change cause status counter/snapshot helper gate
(`view-change-cause-status`),
view-change proof/index status counter helper gate
(`view-change-proof-status`),
leader/highest-QC/locked-QC status projection helper gate (`qc-status`),
TLC-cross-checked validation evidence QC selector helper gate (`validation-evidence-qc`),
TLC-cross-checked pure engine prepare-QC gate,
TLC-cross-checked pure engine exact Prepare-QC lock/highest-QC record gate,
TLC-cross-checked pure engine exact Prepare-QC phase-transition gate,
TLC-cross-checked pure engine Prepare-QC unrelated-state preservation gate,
TLC-cross-checked pure engine prepare-vote cache/output gate,
TLC-cross-checked pure engine commit-QC gate,
TLC-cross-checked pure engine exact Commit-QC highest-QC record gate,
TLC-cross-checked pure engine exact Commit-QC phase-transition gate,
TLC-cross-checked pure engine Commit-QC unrelated-state preservation gate,
TLC-cross-checked pure engine payload-available Commit-QC exact finality gate,
TLC-cross-checked pure engine missing-payload Commit-QC pending/fetch gate,
TLC-cross-checked pure engine Commit-QC validation cleanup gate,
TLC-cross-checked pure engine committed-block gate,
TLC-cross-checked pure engine exact committed-block record gate,
TLC-cross-checked pure engine reconfiguration staging gate,
TLC-cross-checked pure engine reconfiguration activation-height dedup gate,
TLC-cross-checked pure engine committed-block cleanup gate,
TLC-cross-checked pure engine committed-block unrelated-state preservation gate,
TLC-cross-checked pure engine exact payload-availability record gate,
TLC-cross-checked pure engine payload-availability gate,
TLC-cross-checked pure engine payload-availability unrelated-state preservation gate,
TLC-cross-checked pure engine validation-result gate,
TLC-cross-checked pure engine validation-result unrelated-state preservation gate,
TLC-cross-checked pure engine exact validation-owner cleanup gate,
TLC-cross-checked pure engine exact invalid-validation round/output advance gate,
TLC-cross-checked reconfiguration, TLC-cross-checked certified-recovery, TLC-cross-checked view-change, TLC-cross-checked validation-callback,
TLC-cross-checked certificate-admission, TLC-cross-checked highest-QC selection, TLC-cross-checked optional highest-QC selection-filter bounded models,
TLC-cross-checked certified-fetch, TLC-cross-checked pure-engine certificate
dispatch, TLC-cross-checked pure-engine certificate prefilter state,
TLC-cross-checked frontier-gap realignment, TLC-cross-checked Kura commit retry,
TLC-cross-checked missing-block fetch, TLC-cross-checked missing-block hard-cap cleanup,
TLC-cross-checked missing-block hard-cap, TLC-cross-checked missing-block view-change,
TLC-cross-checked native AMX attestation, TLC-cross-checked native AMX receipt validation,
TLC-cross-checked native AMX routing-plan, TLC-cross-checked NPoS VRF epoch seal,
TLC-cross-checked post-commit cleanup, and TLC-cross-checked restart replay candidate-enumeration bounded models,
and updated operator runbooks when defaults change.

## Community and Governance

**Status:** active growth work.

- Use the official X account, [`@hl_iroha`](https://x.com/hl_iroha/), as the
  primary public cadence for recurring X Spaces, demos, and roadmap Q&A.
- Publish recaps or recording links when available so contributors can follow
  progress asynchronously.
- Grow contributor and maintainer diversity by turning testnet interest,
  CBDC/regulated-finance adoption, and LFDT ecosystem connections into repeat
  reviewers and subsystem owners.

**Next checkpoints:** monthly X Spaces cadence, clearer contributor onboarding,
public follow-up notes for LFDT governance review items, and commit/reveal
hardening for SORA Parliament policy juries.
