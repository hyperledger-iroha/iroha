# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-b7bc6c159eeba1194e9f6a6f72f15c442742fa0252d47ab4e060f9af242896c3"></a>

<!-- Original context: Roadmap / SCCP Launch Scope -->
- publish a signed production release-evidence bundle accepted by the Rust
  validator and independently reproduced by release engineering and security.

No fixture key, signal-binding circuit, synthetic receipt, unavailable lane,
or self-consistent proof-controlled roster counts as production evidence.

The signed SCCP V1 production evidence corridor is only Taira↔Ethereum,
Taira↔BSC, Taira↔TRON, and Taira↔TON mainnet. Solana and testnet profiles are
rejected, not dormant alternatives, and cannot satisfy its blockers or
evidence rows. Generic proof jobs/artifacts and retired route-manifest
workflows remain excluded from the first release.



<a id="record-8eb36a1187bde499baac156b91947e411016307054668e8a14d1163b1dd72528"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP V1 production launch scope is limited to Ethereum, BSC, TRON, and TON
  mainnet. Proof policies, checked encoders, verifier dispatch, Torii public
  discovery, SDK helpers, and production readiness surfaces must stay limited
  to those lanes; Solana and TON testnet remain non-production profiles and
  cannot substitute for the required TON mainnet evidence.
  Retired runtime-network families outside that launch scope are explicitly
  unsupported for now.
  SCCP will not support Sub&#115;trate/Pol&#107;adot networks for now.
  Treat that sentence as a current-release support boundary, not a deferred SCCP
  launch task.
  No current source proof, manifest, SDK helper, or Torii route should be
  treated as Sub&#115;trate/Pol&#107;adot-compatible.
  No relayer adapter, route manifest, proof fixture, or public discovery route
  for that network family is part of the current SCCP launch.
  That exclusion is intentional current-launch scope, not a hidden
  compatibility lane.
  Do not track that family as remaining SCCP launch work in this cycle.
  Keep any future compatibility research for that retired family outside SCCP
  launch readiness until governance explicitly re-opens support.
  Torii OpenAPI SCCP discovery descriptions must carry the same no-support
  sentence so relayers see the exclusion before reading proof manifests.
  Release-readiness and strict bundle source inventories now pin both Torii
  OpenAPI SCCP capability/manifest descriptions to that sentence.
  Required Release Evidence must keep the SCCP launch-scope source-inventory
  row itself before public bundle readiness can pass, so supported-domain
  enforcement cannot be hidden by preserving only downstream lane labels.
  Strict release-bundle verifier inventory now also pins the bundle-level
  launch-scope sparse-inventory, missing-gate, required-domain drift, and
  supported/unsupported domain-drift regressions.
  Release-bundle pre-render validation now also emits domain-specific duplicate
  blockers for required, supported-launch, and unsupported-launch domain lists,
  matching strict verifier launch-scope diagnostics before artifacts render.
  Direct all-lanes CLI public-summary validation now emits the same
  duplicate-domain blockers before copied launch-domain roots can be returned,
  including mixed malformed lists with repeated integer domains.
  Copied release-bundle pre-render validation now owns the launch-domain
  integer-list checks in its launch-scope helper and rejects boolean/non-integer
  domain values before exact launch-domain comparison, so `True` cannot alias
  the Ethereum domain in copied `required_domains`, supported-domain, or
  unsupported-domain roots. The standalone all-lanes public-summary helper and
  the release-bundle pre-render helper now also require exact builtin lists before
  iterating or comparing copied launch-domain roots, so hostile list subclasses
  cannot execute hooks or leak copied text before the bounded integer-list
  diagnostics. Standalone all-lanes public-summary roots now also have to be
  exact builtin dictionaries before root, domain-list, lane-list, or checklist
  validation can walk them, so hostile mapping subclasses cannot execute hooks or
  leak copied text before the bounded root-object blocker. Release-bundle
  pre-render mapping/list requirements now mirror that exact-container policy for
  copied report objects and arrays, including copied all-lanes evidence roots, so
  hostile mapping subclasses cannot reach lane binding or Markdown rendering.
  Copied readiness report roots now also keep that exact builtin-dictionary
  boundary between preflight and full bundle validation, so hostile report-root
  subclasses stop at the bounded `bundled report must be an object` diagnostic
  before unknown-field checks or public Markdown rendering can walk them.
  Public release blocker-list helpers in bundle generation and strict
  verification now also require exact builtin lists before iterating copied
  blocker containers, and release-bundle ready/empty follow-on checks only
  inspect exact lists so hostile list subclasses cannot execute truthiness or
  iteration hooks after schema rejection.
  Shared public integer-list helpers now follow the same exact-list rule in
  bundle generation and strict verification before length, duplicate, or item
  checks can inspect copied domain-list containers.
  Release-readiness public JSON rendering now also requires the generated report
  root itself to be an exact builtin dictionary before public-field filtering or
  rendering can walk it, so hostile report-root subclasses stop at the bounded
  `readiness report must be an object` blocker.
  Readiness Markdown rendering in both the report generator and strict verifier
  now follows the same exact-root rule before status checks, canonical ordering,
  or section rendering can walk the report, so hostile report-root subclasses
  produce bounded `NOT READY` Markdown without hook execution or text leakage.
  Readiness Markdown string-list cells and bullets now also require exact
  builtin lists before blocker table cells, top-level bullets, native EVM
  validation blockers, or checklist blocker truncation can inspect copied
  containers.
  Readiness Markdown table renderers now also require exact builtin
  cryptographic-evidence, user-prover, input-artifact, corridor, helper-set, and
  artifact containers before table construction can read or iterate copied data.
  Native EVM prover and source-inventory Markdown tables now apply the same
  exact-container rule to bundle roots, artifact rows, SDK artifacts, inventory
  roots, inventory rows, and gate keys.
  Lane-readiness Markdown rows now also require exact evidence roots, lane
  lists, lane rows, record maps, and blocker lists before rendering lane status
  or blocker cells.
  Release-checklist Markdown rendering now also requires exact checklist roots,
  item lists, and item rows before checklist table construction can inspect
  copied containers.
  Release-checklist schema validation now applies the same exact-container rule
  to public readiness filtering and strict bundle verification before copied
  roots, item lists, item rows, or blocker lists can be walked.
  Mixed copied lane lists now also keep duplicate launch-domain lane
  diagnostics visible when a malformed non-object lane row is present.
  The retired-network surface guard now requires explicit no-support
  launch-scope wording in each launch-scope file, including the exact escaped
  Sub&#115;trate/Pol&#107;adot no-support sentence. Readiness and strict-bundle
  sparse tests now remove every uniquely detectable retired-network surface
  marker, so scan roots, expected file coverage, translated-pipeline coverage,
  specific/generic no-support notes, active-tree forbidden-token scans, release
  gate wiring, and stale allowlist markers cannot silently degrade to sampled
  coverage. The active-tree scan also rejects slash-, colon-, table-, shell-,
  punctuation-, backslash-, whitespace-, zero-width-, HTML-entity-,
  URL-percent-, Unicode-confusable-, compatibility-form-, and
  combining-mark-spliced retired-family names before they can re-enter SCCP
  code, SDKs, scripts, or docs. The decoder now composes HTML-entity and
  URL-percent decoding to stability before matching, with deep adversarial
  coverage for retired-family names and runtime-style tokens.
  Translated public bridge-proof launch-scope docs now carry the same generic
  unsupported-family and not-remaining-work boundary, and the retired-network
  surface guard pins those localized files before release evidence can pass.
  Strict release-bundle verifier inventory now also pins its own retired-network
  guard regressions, while stale retired-network allowlist markers are scanned
  only in the retired-network scan guard so adversarial verifier fixtures can
  quote those strings without weakening the guard. The active-tree scan now
  also rejects separator-obfuscated retired-network names, including hyphen,
  underscore, dot, whitespace-spliced, zero-width-spliced, HTML entity-hidden,
  URL-percent-encoded, Unicode-confusable, fullwidth/compatibility-form, and
  combining-mark forms, so compatibility wording cannot re-enter public SCCP
  surfaces by punctuation, normalization, or homoglyph drift.
  Generated release-readiness Markdown and verifier-owned release-bundle
  Markdown must also carry that exact sentence in the Required Release Evidence
  section before public artifacts can satisfy readiness.
  The strict Required Release Evidence invariant must also require the
  retired-network surface source-inventory row itself, so the sentence cannot
  survive while the release-gate row is silently removed.
	  A generated-output regression now compares the count and rendering of all
	  Required Release Evidence source-inventory row labels against the strict
	  Markdown marker list, so new release gates cannot be added without public
	  Markdown invariant coverage.
  That strict marker list and the generated Required Release Evidence bullet
  list must remain unique, and every marker must be present in the generated
  section, so duplicate marker, exact duplicate bullet, or
  whitespace-normalized duplicate bullet entries cannot hide coverage drift.
  The Required Release Evidence list must also reject noncanonical Markdown
  bullet spelling, including short-indented bullets, extra separator spaces,
  alternate bullet markers, trailing spaces, and internal tab/control-whitespace
  or Unicode separator/format duplicate aliases.
  The top-level Blocking Items Markdown list must follow the same canonical
  bullet and hidden-spacing duplicate rules, so copied public blocker lists
  cannot preserve a blocker marker while adding visually aliased duplicate
  bullets.
  Table-cell blocker lists must also match the exact rendered cell text, so
  release-checklist, native-prover, source-inventory, user-prover, and
  lane-readiness cells cannot add hidden NBSP/zero-width duplicate blocker
  aliases while preserving each blocker substring elsewhere in the section.
  Strict readiness Markdown and release-notes heading parsing must also reject
  noncanonical top-level title/status blocks, non-exact public section-heading
  spelling, unexpected public section headings, Setext headings, repeated
  public section headings, and noncanonical required-section order, with
  inventory-pinned regressions so duplicate titles, conflicting statuses,
  padded headings, inserted, short-indented, Setext-underlined, duplicated, or
  swapped headings cannot preserve marker coverage while changing the public
  artifact structure.
	  Release-notes artifact paths must also stay decoded before sensitive-marker
	  checks, and strict artifact-row diagnostics must redact decoded sensitive
	  marker rows plus decoded control or non-ASCII rows before reporting
	  unexpected public release-note artifact rows.
	  Release-notes attachment blocker bullets now also require exact builtin
	  blocker lists before rendering the Blocking Items section, so copied list
	  subclasses cannot execute hooks or leak text into public Markdown.
	  Release artifact paths, copied evidence/native manifest filenames, phase
  evidence paths and directories, output paths, native prover manifest paths,
  manifest artifact paths, and adjacent copied public schema keys must all use
  decoded sensitive-marker classification before public JSON or Markdown can
  preserve encoded secret-looking labels.
  Top-level release-bundle and standalone readiness-report CLI exception
  redaction must use the same decoded sensitive-marker classification before
  stderr can preserve helper failure details.
  They must also apply decoded unsafe-text checks before preserving otherwise
  safe details, so encoded control characters, non-ASCII/RTL text, pipes, or
  angle brackets fall back to the fixed generation-failed category.
  The standalone readiness-report CLI now also shares the release-bundle
  placeholder-aware scan, so safe `<path>`/`<invalid ...>` redaction placeholders
  can be preserved while sensitive or Markdown-unsafe placeholder labels still
  fall back to the fixed generation-failed category. The release public
  scalar-text source inventory now pins those safe and hostile placeholder-label
  regressions for both top-level release CLIs.
  The readiness generator must also compare emitted `source_inventory` keys
  against the strict verifier required-gate set so verifier-only gates cannot be
  omitted from generated report JSON.
  Reintroducing any such family requires a new design pass, fresh fixtures, and
  explicit governance approval rather than reviving diagnostic code paths.
  Rust proof-manifest production readiness must also reject contradictory
  disabled metadata: a manifest that claims `production_ready` cannot carry
  `disabled_reason`, and the core gate must stay pinned against schema version,
  production flag, chain/domain labels, canonical destination-binding key/hash,
  proof-family, backend/finality metadata, manifest seed, submission template,
  and verifier-target drift. Release-readiness and strict-bundle source
  inventories must pin those Rust gate markers alongside generated manifest
  readiness flags. Source-adapter deployment readiness must also keep the
  governed descriptor metadata fail-closed: V1 schema, source-chain label,
  source-proof plan, finality model, adapter proof family, and adapter circuit
  id drift must not match material or open source-adapter readiness. The
  Ethereum EVM source-adapter deployment source inventory must pin every Rust
  descriptor metadata-drift assertion beside source-bridge network/config and
  receipt replay checks. That EVM deployment inventory must also keep explicit
  ETH and BSC lane sentinels, including SDK deployment helpers and replay
  regressions, so one EVM launch lane cannot silently lose deployment evidence
  coverage while the other lane keeps the gate passing. Ethereum source-bridge
  config hashing now rejects network-id/code-hash role reuse at the direct
  config-hash helper and source-material constructor layers, and the Python
  source-bridge evidence path mirrors that fail-closed guard before rendering
  material hashes or TOML. The JS, Python client, Swift, Kotlin, Java Android,
  and C# SDK source-material normalizers now reject the same Ethereum
  source-bridge network-id/code-hash role reuse before config-hash mismatch
  diagnostics. The release-readiness plus strict-bundle source inventory pins
  those Rust/Python/SDK adversarial guards beside the cross-SDK config checks.


<a id="record-27376efb429f08afff7be6debf76a0ac9e956951e793e35d5b6cf0e48312e127"></a>

- SCCP TRON route-config production blockers must stay fail-closed at the
  route-manifest boundary. Release-readiness and strict release-bundle source
  inventory now pin the post-deploy blocker key list and adversarial route
  overlay tests for source-event, route-canary, full-TOML, generic
  post-deploy, scalar, malformed, and contradictory blocker evidence. Strict
  bundle sparse regressions must keep the full-TOML production-blocker marker
  pinned alongside route-canary and generic post-deploy blocker markers, and
  both public gates must pin the adversarial input markers themselves rather
  than only their expected diagnostic regexes. The BSC route-config gate must
  keep the same named source-event, post-deploy, full-TOML, and route-canary
  adversarial blocker cases pinned in readiness and strict-bundle sparse
	  regressions, and both public gates must pin the BSC/TRON deploy-script
	  blocker keys, canonical route-manifest normalizers, and governed route
	  metadata checks. BSC/TRON `route-manifest` commands must reject `--out`
	  paths that resolve to deployment evidence, TAIRA burn-record contract
	  material, verifier/prover material, live evidence, offline full-TOML
	  evidence, or browser-prover manifests before parsing inputs or writing the
	  manifest, and the BSC route-manifest sparse inventory now pins the
	  symlink-parent alias regression directly. BSC/TRON deploy commands must
	  reject deployment evidence/plan
	  `--out` paths that resolve to verifier material or deployer secrets,
	  including through symlink-parent aliases, before parsing operator inputs,
	  loading signers, or reaching networks. TRON `sign-transaction` and
	  `broadcast` must also reject `--out` paths that resolve to source
	  transaction artifacts or deployer secrets, including through symlink-parent
	  aliases, before signing, broadcasting, or writing output, and the
	  release/readiness inventory must pin those guards beside the offline
	  regressions. BSC native-prover bundle
	  generation must also reject bundle `--out` paths that resolve to governed
	  proof, proving-key, verifier-key, Groth16 material, self-test, parity, or
	  SDK implementation artifacts, or file-backed audit evidence, including
	  through symlink-parent aliases, before writing the bundle. The shared
	  BSC/TRON route-output guards and TON
	  route-manifest guards now also compare existing real filesystem targets
	  and symlinked parent directories, so symlink-parent output aliases cannot
	  overwrite governed manifests, route configs, deployer secrets, deployment
	  evidence, native-prover bundles, or Groth16 material inputs while existing
	  NUL/unsafe-path diagnostics remain owned by the artifact-path validators.
	  TON route-manifest JSON output also writes by exclusive temp-file rename
	  so final output symlinks are replaced instead of followed and hostile temp
	  symlinks are retried without receiving artifacts. TRON deploy-helper,
	  BSC deploy-helper, and BSC Groth16 public JSON/text output writes also use
	  randomized, exclusively-created temp files so predictable temp-path
	  symlinks cannot receive artifacts. BSC Groth16 `generate` must reject
	  generated circuit/setup, verifier-key, manifest, copied transcript, and
	  local Powers-of-Tau output paths that resolve to operator-provided source
	  inputs or to each other, including through symlink-parent aliases,
	  before creating the output directory, copying artifacts, running Circom, or
	  invoking SnarkJS. BSC Groth16 `toolchain-fingerprint` must reject copied
	  transcript `--out` paths that resolve to the source transcript, including
	  through symlink-parent aliases, before parsing it or hashing local tools.
	  BSC Groth16 `proof-self-test` must reject report `--out` paths that
	  resolve to the material manifest, witness WASM, or manifest-bound
	  circuit/proving/verifier artifacts, including through symlink-parent
	  aliases, before parsing sentinel inputs, invoking SnarkJS, or writing
	  reports. BSC Groth16
	  `materialize` must reject copied input artifact target collisions and
	  copied-input collisions with fixed verifier-key or manifest outputs,
	  including through symlink-parent aliases, before copying artifacts or
	  writing production material. BSC Groth16
	  `transcript-template` and `evidence-template` must reject generated output
	  path collisions with their own output sets or source artifacts, including
	  through symlink-parent aliases, before replacing any transcript, evidence,
	  report, index, or manifest files. BSC
	  Groth16 `attestation-request` must also reject request
	  `--out` paths that resolve to the material manifest, semantic review
	  evidence, or circuit-security audit evidence, including through
	  symlink-parent aliases, before parsing those artifacts or writing the
	  unsigned request package, and `handoff-bundle`
	  must reject `--out` paths that resolve to the material manifest,
	  transcript-template package, evidence-template package, or attestation
	  request package, including through symlink-parent aliases, before parsing
	  those artifacts or writing handoff JSON.
	  BSC Groth16 `sign-attestation` must reject signed-role `--out` paths that
	  resolve to the unsigned attestation request package or Ed25519 private key,
	  including through symlink-parent aliases, before parsing those inputs or
	  writing the signature artifact.
	  `finalize-attestations` must reject explicit `--out-dir` paths that
	  resolve to the unsigned request package or signed role attestation files,
	  including through symlink-parent aliases, before parsing signed inputs or
	  materializing production-ready outputs.
	  BSC/TRON
	  route-config blocker-list validators must also
	  keep handoff-placeholder regressions free of open-work markers while still
	  rejecting explicit placeholder, to-do/deferred-work, dummy, fixture, mock,
	  sample, stub, replacement, and `your_*` wording before any production-ready route
	  manifest can be rendered.
	  Those blocker-list validators must also
	  decode HTML entities and bounded URL-percent encodings before
	  sensitive-name matching, including decoded whitespace separators such as
	  `api token` and `private key`, so encoded secret/token/private-key blockers
	  fail closed with fixed public diagnostics, and duplicate checks must use the
	  same decoded/lowercased key so encoded copies cannot repeat operator blocker
	  text. The same decoded boundary must reject encoded control characters and
	  non-ASCII/RTL text before route-config diagnostics or generated TOML can
	  preserve post-deploy blocker text. Standalone all-lanes public-summary
	  sanitizers must keep the same
	  decoded sensitive-name treatment for recovery-phrase blockers and copied
	  nested lane values. Decoded sensitive-name checks must also normalize common
  Greek/Cyrillic homoglyphs before matching so encoded lookalike secret,
  token, private-key, or recovery-phrase labels cannot be echoed through
  all-lanes summaries, readiness reports, release-bundle diagnostics, strict
  verifier diagnostics, release-note artifact rows, copied public paths, or
  lane CLI exception details, including double-percent-encoded helper text and
  double-HTML-entity sensitive/control text. Lane CLI redaction tests now also
  pin five-layer URL-percent encoded `api key` and Markdown-unsafe pipe details,
  plus deeply nested HTML-entity private-key and pipe markers, across every lane
  helper and the strict source inventory. Top-level release-bundle and
  standalone readiness CLI error-detail regressions now pin the same deep unsafe
  and sensitive payloads before preserving otherwise safe operator diagnostics.
  The all-lanes TOML parser must treat mnemonic,
  recovery-phrase, and seed-phrase duplicate-key or section labels as sensitive
  category-only diagnostics. Native EVM prover bundle duplicate-key diagnostics
  must also decode bounded URL/HTML forms before sensitive-name matching in the
  release-bundle builder, readiness-report helper, and strict verifier, with
  strict source-inventory pins for the no-leak and marker-family regressions.
  Release-bundle,
  standalone readiness, and strict verifier copied SDK-result, source-inventory,
  expected-crypto-row, and semantic audit-role mapping traversal must stay on
  type-only safe key ordering so hostile non-string public keys cannot raise
  during residual schema checks. Generated all-lanes source-adapter gate
  hash-role checks must use that same ordering before role-reuse comparison,
  and the source-material role-validation inventory now pins the generated
  all-lanes audit-hash ordering marker.
  The TRON runtime
  route-manifest
  gate must also pin the Rust
  parser, post-deploy evidence validator, Base58 normalizer, production metadata
  diagnostics, and adversarial route-manifest regressions in readiness and
  strict-bundle sparse tests. Those sparse tests must remove every uniquely
  detectable runtime route-manifest marker across Rust validation, readiness
  gate wiring, strict verifier wiring, and self-inventory rows.


<a id="record-94bffed8b7ac1edc98e5fec23402def9572a7641ce79e1564961bb275814a4c2"></a>

- SCCP destination evidence reparse diagnostics must stay category-only before
  public TOML blockers are emitted: EVM copied bridge/verifier runtime bytecode
  evidence and TON copied verifier code BoC base64 evidence must not propagate
  raw parser details or operator-provided bytes into readiness artifacts.
  EVM copied runtime-bytecode reparsing must convert helper `SystemExit`,
  `RuntimeError`, `TypeError`, and `ValueError` failures into the fixed TOML
  evidence blockers for both bridge and verifier runtime roles.
  The EVM destination helper must keep separate adversarial copied-bytecode
  regressions for the bridge and verifier runtime roles, both pinned by
  release-readiness and strict bundle source inventory.


<a id="record-e05d0e82227a92e738ed6ed9f9043f6a9a2a385d12b4a16640b3346ff0189b79"></a>

- SCCP imported EVM live summary reparse diagnostics must stay category-only
  before public TOML blockers are emitted: source bridge, destination bridge,
  and destination verifier runtime bytecode metadata must not propagate raw
  parser details or operator-provided bytes from copied live evidence. EVM live
  bridge/verifier runtime-bytecode and bridge-address imported metadata
  reparsing must convert helper `SystemExit`, `RuntimeError`, `TypeError`, and
  `ValueError` drift into fixed metadata blockers before TOML rendering; the
	  EVM source-live source bridge runtime-bytecode and source bridge address
	  imported metadata path must keep the same category-only handling.
	  EVM live destination imported metadata must also keep exact integer
	  `expected_rpc_chain_id`, `source_domain`, and `target_domain` checks so
	  boolean copied values cannot alias ETH/SORA lane IDs before full-TOML
	  rendering. EVM source-live imported metadata must keep the same exact
	  `expected_rpc_chain_id` rule and exact ETH-domain prerequisite selection.
	  Direct EVM live/source-live default-domain helpers must reject boolean
	  domains before selecting default RPC chain IDs or block tags. Raw all-lanes
		  EVM live metadata must also keep the exact per-domain block-tag policy:
		  Ethereum source/destination evidence uses `finalized`, BSC source/destination
		  evidence uses `latest`, and copied BSC summaries cannot replace that with
		  forged `finalized` or arbitrary non-empty strings.
			  Verified EVM source-live TOML acceptance must cover both ETH and BSC
			  all-lanes imports, preserving the canonical BSC source bridge profile,
			  `latest` block tag, and recomputed BSC source/route/canary hashes before
			  production readiness can pass.
			  Verified EVM destination-live TOML acceptance must keep the same
			  ETH/BSC all-lanes parity: ETH route-canary evidence remains
			  `finalized`, while BSC canonical `latest` reads bind the route-canary
			  under the `bsc_latest` policy and render full destination evidence.
			  EVM receipt-proof mainnet chain validation must also reject boolean domains
			  and boolean expected chain ids before any JSON-RPC call.
	  Receipt-proof source-event mode selection must keep rejecting the removed
	  `allow_receipt_only_evidence` keyword and removed
	  `--allow-receipt-only-evidence` flag before JSON-RPC, while requiring a
	  source bridge for every emitted receipt proof summary.
	  Receipt-proof fixed-hex parser helpers must likewise require exact boolean
	  `nonzero` controls before zero proof components can be accepted or rejected.
	  EVM live fixed-hex parser helpers must keep the same exact boolean
	  `nonzero` controls before zero route-canary or live metadata fields can be
	  accepted or rejected.
	  EVM source-live RPC fixed-hex parser helpers must keep the same exact
	  boolean `nonzero` controls before zero deployment receipt fields can be
	  accepted or rejected. EVM source-live CLI parser helpers must also require
	  exact string values for domain, component hashes, expected chain ids, and
	  block tags before any string-like helper method can run.
	  EVM destination-live CLI parser helpers must keep the same exact string
	  boundary for bridge/component hashes, bridge addresses, expected chain ids,
	  and block tags before any string-like helper method can run.
	  Solana live CLI parser helpers must keep the same exact string boundary for
	  verifier program ids, ProgramData slot pins, and bytes32 evidence hashes
	  before any string-like helper method can run.
	  TON live CLI parser helpers must keep the same exact string boundary for
	  verifier raw addresses and bytes32 evidence hashes before any string-like
	  helper method can run.
	  TRON live CLI parser helpers must keep the same exact string boundary for
	  TRON address payloads and bytes32 evidence hashes before any string-like
	  helper method can run. TRON live decimal and enum parser helpers must also
	  reject string subclasses before source-domain, protobuf-int, or transaction
	  enum canonicalization can invoke comparison, indexing, ASCII, decimal, or
	  hash hooks.
	  TON destination CLI parser helpers must keep the same exact string boundary
	  for verifier raw addresses, code BoC text/file path inputs, account status,
	  last-transaction LT text, and bytes32 evidence hashes before any string-like
	  helper method can run.
	  EVM live route-allowlist recomputation must also require an exact boolean
	  `include_route_canary` gate before including or omitting route-canary
	  evidence from the recomputed summary.
	  Source-live deployment receipt readiness must also keep imported source
	  bridge and receipt contract-address parser `SystemExit`/`RuntimeError` drift
	  inside the fail-closed readiness predicate, and must compare the guarded
  receipt block-hash parse rather than reparsing after the sanitized block.
  Source-live deployment receipt field parsing must also classify `TypeError`
  failures as fixed receipt-field categories before CLI or release output can
  expose parser exception details.

