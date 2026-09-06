# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-72dee5c8a54c5d7db1d5a57a1f8fbcdc4bef5584f1f141daeb26c78177d7605d"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- The focused SCCP prover corridor is green for the current production-hardening
  slice across JavaScript, Python, Swift, Kotlin/JVM, Java Android, the Rust
  `iroha_sccp` verifier crate, core bridge-proof admission tests, and on-chain
  EVM/TRON Groth16 contract smoke coverage for post-generation payload,
  finality-height, and finality-block public-signal drift.


<a id="record-c3453e89b7f4c791c2f27b4c1d6f96f9665ce2a75529a301d8c5704f8e35f657"></a>

- Supported EVM/BSC, TRON, Solana, and TON user-prover readiness rows now
  include per-SDK helper symbol maps for JavaScript/web, Python, Swift,
  Kotlin/JVM, and Java Android. Those maps carry the native source-proof,
  source-state, or full-light-client audit proof-generation helpers where
  applicable alongside the final proof request and submission helpers. Release
  bundles therefore cannot claim the portal/mobile native proof paths without
  explicitly carrying the UI proof-generation surfaces for each consumer SDK;
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
  explicit `proofResult: null` / `proof_result=None` instead of treating it as
  an omitted proof result, keeping null/omitted semantics aligned with Solana
  builders across JavaScript, Python, Swift, Kotlin/JVM, and Java Android now
  also reject non-empty standalone `sourceProofBytes` unless a wrapped
  `proofResult` is supplied, because the final runtime-call payload carries the
  recursive bundle but not those request-bound source-proof bytes. The
  tracked JavaScript `dist/` package artifact is regenerated from that source
  and the package-dist suite now exercises the published `dist/index.js`
  portal SDK artifact aligned with the source guard. Public readiness reports
  now also require the
  JS corridor transcript to include the source SCCP tests, `package_dist`, and
  package export tests in the claimed `js-sdk` phase, so a release bundle cannot
  prove only source-side helper tests while omitting the dist artifact surface
  counterparty package builders now apply the same native recursive payload cap
  to canonical bundle bytes before emitting `SolanaProgramInstruction`,
  tooling and portal/mobile SDKs on the same submission corridor.


<a id="record-45885a7af1baeb1beacc8bdb04067031f4df0bc93179f701bc8c3673429b66bd"></a>

- EVM route-canary evidence now uses a v4 transcript aligned with the TRON
  hardening model: ETH/BSC canary hashes bind the receipt block
  number/hash/`receiptsRoot`, submitted calldata SHA-256, decoded
  payload/finality public inputs, proof version/source domain, target domain,
  consumed-message state, and finalized receipt-block readback flag before
  all-lanes preflight or Rust `iroha_sccp` route admission can mark route
  evidence launch-ready. TRON live
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
  java-tron transaction Merkle leaves for source proofs. The EVM/BSC v3
  route-canary fields, including the receipt block number/hash/`receiptsRoot`,
  are also first-class config and ZK policy-hash material, keeping Core/Torii
  configured admission bound to the same calldata, payload, finality, and proof
  transcript that `iroha_sccp` validates. The local non-Windows SCCP
  production-corridor phases are revalidated with Rust SCCP verification,
  operator evidence scripts, JS/Python/Swift/Kotlin/Java Android SDK prover
  surfaces, EVM/TRON contract smoke, and core bridge-proof admission. The
  Windows `.NET 8` SDK TRX evidence remains the host-certification blocker for
  a complete corridor, alongside governed/live deployment evidence.
  Route-canary readback remains paired with the finalized head and runtime
  versions in public readiness JSON; release-bundle verification rejects zero
  or governed-hash-reused finalized-head/runtime-code canary fields before
  release notes can pass.


<a id="record-4970bd0b9ef05e3d0511f5a1c00f3a158ae315e59caeb4c8b5925c43176454a0"></a>

- The focused SCCP production corridor is now captured by
  `scripts/check_sccp_production_corridor.sh`, with phase selection for the
  Rust verifier crate, operator evidence scripts, web/Python/Swift/Kotlin/Java
  Android SDK proof generators, native .NET/C# ETH/BSC facade tests, the
  BSC/TRON deployment evidence tests, EVM/TRON Groth16 contract smoke, and core
  bridge-proof admission target. Release transcript gates now also require
  phase-local Node zero-failure output plus named success output for the BSC
  deploy/config test, TRON route-manifest deployment-evidence test, and shared
  TAIRA XOR contract test before `contract-smoke` evidence can pass. The same
  transcript gate rejects phase-local failure summaries, including mixed
  `failed`/`passed` pytest output, non-zero Node failure counts, failed Cargo
  summaries, Gradle `BUILD FAILED`, Swift failure counts, and failed .NET
  summaries, rather than trusting a positive success substring alone. It also
  rejects duplicate claimed phase markers so a clean first phase block cannot
  hide a later duplicate failed block in the same release artifact. The
  full-corridor completion fallback now requires every phase block to carry its
  own traced commands, success markers, and failure-free output, so marker-only
  full-corridor stubs cannot satisfy a per-phase release artifact. Completion
  sentinels must also be exact output lines, not substrings embedded in other
  output, must appear after the commands and success output they certify, and
  must be terminal for non-empty output in the completed transcript. Only exact
  known corridor phase markers may delimit phase blocks, and non-empty output
  before the first phase marker is rejected;
  prefix-like marker output is a blocker instead of a way to hide later failure
  lines. Any transcript containing multiple exact known phase markers must
  satisfy the full-corridor validator, so partial multi-phase logs cannot pass
  as complete single-phase evidence, and full-corridor logs must keep the
  production runner's canonical phase order. The
  Java Android phase now matches the current
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
  contains the input TOML or phase transcript sources; existing non-directory
  output paths are rejected before forced replacement as well, preventing
  evidence loss during release packaging. Successful production-ready bundle generation now
  prints the verified `manifest_sha256` root directly, and reviewers can run
  `scripts/sccp_verify_release_bundle.py` against the published bundle to
  recompute every attachment hash, emit the verified `manifest_sha256` root for
  archival release review, and catch extra manifest artifacts that are not
  referenced by the readiness report, unknown corridor evidence phase keys,
  skipped or missing required corridor phases hidden behind top-level ready
  flags, non-canonical phase-log destinations, copied TOML evidence drift,
  tampered logs, symlinked manifests or artifacts, unsafe manifest paths,
  unmanifested or omitted required/phase artifacts, non-canonical
  manifest/readiness-report/summary JSON serialization, duplicate keys and
  malformed duplicate-key names in public JSON roots, manifest artifact-order
  drift from the bundle builder's
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
  missing-record lane flags, blocked release-checklist items, duplicate
  release-checklist gate ids in report/embedded-evidence/summary roots,
  duplicate public blocker strings, malformed all-lanes lane
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
  finalized-head/runtime-code hashes,
  TRON zero owner/recovered route-canary addresses, zero transcript words, zero
  route-canary binding hashes, reused canary hash roles including
  finality-height replay, or recovered signer drift from the transaction owner,
  route-canary route/destination hash drift from sibling lane evidence,
  zero cryptographic evidence row hashes, cryptographic evidence row
  domain/chain or per-field source/destination/source-gate/route/canary drift from
  embedded lane rows, unknown
  manifest
  or report artifact fields, zero or malformed artifact byte counts, malformed
  artifact hash JSON types, malformed
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
  supported production lane, distinguishing EVM/TRON Torii bridge-proof submit
  payloads from native Solana instruction and TON BOC envelopes that
  networks are outside launch scope. Each surface row uses the
  user-side proof backend labels consumed by the SDK request builders
  (`sccp-solana-recursive-mainnet-v1`, `ton-contract-v1`,
  `evm-groth16-bn254-v1`, and `tron-groth16-bn254-v1`) and is tied back to the
  required JavaScript, Python,
  Swift, Kotlin, Java Android, and core-admission corridor phases, with EVM/TRON
  additionally requiring contract-smoke coverage. The Solana destination
  manifest still binds the `solana-program-v1` target verifier backend, while
  the user-prover surface advertises the recursive backend id consumed by
  browser/mobile proof requests; release-bundle verification now rejects any
  blocked submission-surface row or non-empty validation blocker before the
  surface can be published as validated.

