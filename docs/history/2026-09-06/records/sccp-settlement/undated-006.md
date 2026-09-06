# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-c8cf5523e86a77293843db15083d13afa6d5281645273ea652f78f6d3e002962"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP BSC TAIRA XOR route-config generation must reject contradictory
  post-deploy readiness: production-ready route manifests cannot carry non-empty
  `postDeployLiveEvidence` production blocker arrays, and malformed blocker
  containers must fail closed before a governed Torii overlay can be rendered.
  Route-manifest JSON string fields must also be canonical before route-config
  normalization: surrounding whitespace in route ids, asset keys, network ids,
  post-deploy transaction ids, offline TOML hashes, uppercase bytes32 metadata,
  uppercase or `0X` EVM address metadata, and non-lowercase `bscNetwork`,
  `chain`, or `chainIdHex` values are rejected instead of being normalized into
  accepted production metadata. Optional manifest-owned free text, including
  `disabledReason`, `settlement.contractAddress`, and
  `settlement.contractAlias`, must also reject surrounding whitespace,
  non-string values, and contradictory snake_case/camelCase aliases before TOML
  rendering; snake_case settlement aliases are accepted only when they render to
  the same exact canonical TOML values. Route-config generation also requires
  production-ready BSC manifests to carry profile-bound `explorerUrl` and
  `explorerHost` metadata, while disabled legacy drafts can be backfilled to
  the selected profile and contradictory explorer aliases still fail closed.
  Production-ready BSC manifests must also reject own-key and non-opaque string
  handoff placeholders (`to-do`, `example`, `replace-me`, `changeme`, `sample`,
  `stub`, `test-only`, `your-*`) before route-config rendering or canonical
  production-output publication. The same handoff-placeholder scan applies to
  canonical BSC deployment evidence and native prover bundle artifacts under
  `artifacts/sccp-bsc`.
  Release-readiness and bundle verification now pin those BSC route-config
  implementation, handoff-placeholder guard/test markers, exact
  uppercase-network, `0X` chain-id, uppercase post-deploy transaction,
  uppercase offline-TOML, and optional text adversarial-test markers, plus
  post-deploy, full-TOML, source-event transaction, route-canary blocker
  contradiction, scalar, malformed-entry, and explorer-metadata markers, as a
  required source-inventory gate before production evidence can pass. The
  readiness-report and strict bundle sparse inventory tests must remove every
  BSC route-config marker across deployment scripts, canonical manifest
  validators, post-deploy blocker extraction, route/TOML field normalization,
  settlement aliases, and adversarial manifest tests.
  Browser-prover references in BSC route manifests must follow the same
  duplicate-alias boundary before route-config TOML is generated: module URL,
  optional module specifier, module hash, manifest hash, route hash, and proof
  hash aliases cannot be copied into the same nested prover record, even when
  the values agree. Browser-prover sidecar JSON must reject duplicate
  module-specifier aliases at route-manifest generation time under the same
  canonical-manifest inventory gate. Present malformed aliases must be treated as
  malformed canonical strings rather than absent values, so null or scalar
  snake_case browser-prover hash aliases cannot be hidden behind valid camelCase
  route fields. Scalar route-config readers must also skip accessor-backed
  preferred aliases before later own data aliases, so a hostile getter cannot
  mask valid snake_case production-ready, domain, gas-limit, or full-TOML
  readiness values or run during manifest normalization.
  Runtime route-manifest admission must also reject deployment evidence hash
  replay from verifier code, verifier key, destination binding, proof artifact,
  or proving-key hashes before a BSC/TRON route can be marked production-ready;
  parser and route-manifest ISI regressions must keep those hash roles distinct.
  Runtime route-manifest admission must also reject copied post-deploy evidence
  roles before BSC/TRON production routes can mutate state:
	  `post_deploy_route_canary_evidence_hash` cannot replay
	  `post_deploy_source_bridge_config_hash`, and
	  `post_deploy_route_canary_transaction_id` cannot replay
	  `post_deploy_source_event_transaction_id`. TRON runtime ISI coverage must
	  pin the same production-ready boundary at state mutation time: the canonical
	  mainnet `taira_tron_xor` manifest can be inserted, while wrong route ids,
	  wrong domains, and copied post-deploy canary hashes are rejected without
	  replacing existing registry state.
	  BSC runtime route-manifest admission must keep browser-prover sidecar
	  `module_hash`/`manifest_hash` roles distinct from verifier, destination
  binding, proof, proving-key, native-prover, deployment-evidence, and sibling
  browser-prover hashes before an on-chain route update can replace governed
  route material.
  Runtime browser-prover `module_specifier` text must also stay canonical:
  present values are non-empty and unpadded, matching generated manifest
  validation instead of accepting runtime-only trimming.


<a id="record-176da9e8ed9e5d92a0ca90f377191d771b2b772b55baed2af89585919b2f3574"></a>

- SCCP TRON TAIRA XOR route-config generation follows the same canonical
  manifest text policy before TOML rendering. Padded route ids, asset keys,
  network ids, destination rollout network ids, post-deploy transaction ids,
  and offline TOML hashes, plus uppercase bytes32 metadata, are rejected as
  malformed manifest input rather than being normalized into accepted route
  metadata. Non-lowercase `tronNetwork`, `chain`, and `chainIdHex` manifest
  values are rejected at the same boundary. Optional manifest-owned free text,
  including `disabledReason`, `settlement.contractAddress`, and
  `settlement.contractAlias`, must reject surrounding whitespace, non-string
  values, and contradictory snake_case/camelCase aliases before TOML rendering;
  snake_case settlement aliases are accepted only when they normalize to the
  same exact canonical text. Required route-manifest container and scalar
  aliases must not appear in both camelCase and snake_case forms, even when the
  values match, before TOML rendering can continue, including fixed TRON
  addresses, destination rollout/binding domains and hashes, burn-record
  VK/artifact/hash material, destination verifier aliases, post-deploy evidence
  hashes, and settlement route/submit aliases. Production-ready TRON
  manifests must also reject
  own-key and non-opaque string handoff placeholders (`to-do`, `example`,
  `replace-me`, `changeme`, `sample`, `stub`, `test-only`, `your-*`) before
  route-config rendering. Route-config normalization must also read manifest
  fields through own data-property descriptors only, so accessor-backed aliases
  cannot be counted as duplicate data, required accessor-only fields are
  absent, and recursive secret, placeholder, and blocker scans cannot invoke
  getters. Release-readiness and bundle verification now pin those TRON
  route-config implementation, duplicate-alias, accessor-backed-alias, and
  handoff-placeholder guard/test markers, and adversarial-test markers as a
  required source-inventory gate before production evidence can pass. The
  readiness-report and strict bundle sparse inventory tests must remove every
  TRON route-config marker across deployment scripts, canonical manifest
  validators, post-deploy blocker extraction, route/TOML field normalization,
  settlement aliases, and adversarial manifest tests.


<a id="record-b2f4f22aefad88e0fdebaa9cced938da64b6548ddd4ccfbe7352c9313fe21bf2"></a>

- SCCP active-launch required-record metadata must stay exact: release notes
  cannot report the active required-records item ready unless the normalized
  lane summary is domain `1`, chain `eth`, production-ready, and each required
  record flag is boolean `true` with no unknown record fields. Stringified
  domain ids, padded chain labels, and stringified production-ready flags are
  pinned as adversarial blockers in both readiness and strict bundle tests.
  Required record flags must also reject copied truthy strings, numeric values,
  `false`, and missing/null values in both readiness and strict bundle
  recomputation. The active checklist schema inventory now pins the exact
  missing launch-lane, chain, production-ready, source-material,
  source-adapter, destination-rollout, and route-allowlist blocker strings
  beside those tests, so the release gate cannot degrade to function-name-only
  required-record coverage.
  Unknown required-record summary keys must be schema-classified before checklist text
  is rendered, preserving safe operator diagnostics while padded,
  control-character, whitespace, Markdown-unsafe, malformed, or
  Unicode-confusable keys become category-only blockers.


<a id="record-7a026455ff56da4afff4ba41ddc7af97020031765b1821a94afd8ee6cc418445"></a>

- SCCP active-launch unresolved-blocker metadata must stay lane-local:
  release notes cannot report the no-unresolved-blockers item ready if the
  active lane carries lane-local blockers, malformed blocker containers, or
  non-string/empty blocker entries even when the top-level aggregate blocker
  list is missing those entries.
  The governed-deployment, route-allowlist, and live-route-canary checklist
  buckets must also fail closed on malformed active-lane blocker containers
  before category matching runs, so scalar, padded, or non-string entries cannot
  disappear from category readiness while only the aggregate blocker gate fails.
	  The active no-unresolved-blockers collector must apply the same canonical
	  string policy to embedded evidence root blockers and active-lane blockers, so
	  empty, padded, non-string, or duplicate entries remain schema diagnostics
	  rather than unstructured or repeated blocker text. Numeric, null, and
	  duplicate blocker entries are pinned in
	  readiness and strict bundle inventory checks for the active no-unresolved
	  blocker collector. The checklist schema inventory now also pins the
	  repeated-numbering regressions for active-lane duplicates, malformed
	  active blocker issue families, and root/lane duplicate groups.
  It also pins the component-level duplicate blocker regressions for route
  canary, route allowlist, destination rollout, and source-adapter gate blockers,
  so component blocker duplicate handling cannot disappear behind aggregate
  active-lane duplicate coverage.


<a id="record-d764d74bceea8bc3c0c93f48e998b0d19bbc649cb92593e0fcac939f3e887a39"></a>

- SCCP release-bundle public string-list fields must reject trim-normalized
  evidence. Manifest, readiness-report, corridor, release-checklist, embedded
  evidence, standalone all-lanes, and lane blocker arrays must contain
  non-empty strings with no surrounding whitespace, so padded blocker text or
  helper-symbol names cannot pass schema validation by being trimmed later.
  Bundle-builder and strict-verifier public schema helpers must require exact
  boolean `allow_empty` controls for string lists, public blocker lists, integer
  lists, and scalar strings before empty copied public fields can be accepted.


<a id="record-c4c3a865e7cec09226705a33166bc0522bec7648cf015f2fbb33232be22826f8"></a>

- SCCP release-bundle public scalar string fields must follow the same
  canonical-text rule. Release-checklist ids/titles, cryptographic-evidence
  chain and route-canary source labels, user-prover submission surface text,
	  all-lanes lane chain labels, destination-binding keys, and route-canary
	  status/source fields must reject surrounding whitespace instead of relying on
	  later normalization. They must also reject control and non-ASCII text,
	  Markdown-unsafe scalar text after URL/HTML decoding, and encoded sensitive
	  names before public Markdown/JSON release evidence is emitted or verified.
	  The scalar-text schema must stay pinned as a readiness source-inventory gate
	  before published bundle readiness can pass. Exact
	  padded-value regressions for release-checklist titles, all-lanes chain labels,
  destination-binding keys, route-canary status/source fields, cryptographic
  route-canary source labels, and submission-surface text are now source-inventory
  markers. The scalar-text inventory must also pin direct lane sentinels for
  `eth`, `bsc`, `sol`, `ton`, and `tron` so dropping one launch lane's parser or
  redaction coverage cannot be masked by the remaining scalar validators.
  Readiness-report and strict bundle sparse inventory checks must remove
  every uniquely detectable public scalar-text marker across verifier schema
  checks, live-evidence helper diagnostics, all-lanes scalar extraction,
  release-bundle/preflight schema checks, adversarial redaction tests,
  copied-corridor/crypto/submission regressions, and self-inventory rows.
  Release-checklist item ids must also stay in the fixed public gate set and
  classify malformed ids before duplicate, drift, or Markdown-presence checks.
  Release-checklist root and item unknown fields must also use structured
  malformed-name diagnostics, preserving safe operator field names while
  blocking raw malformed public key echoes. The bundle builder must classify
	  malformed copied release-checklist root/item unknown fields before render,
	  reject unknown release-checklist root or item fields, malformed item ids/titles,
	  duplicate item ids, non-exact ready booleans, and noncanonical or non-empty
	  ready-item blockers before `--allow-not-ready` diagnostics can render or write
	  public artifacts. Copied readiness-report release checklists must also
	  recompute from embedded all-lanes evidence plus native prover status before
	  rendering, so syntactically valid item drift cannot publish before strict
	  verification.
  Bundle-builder and strict-verifier release-checklist helper controls such as
  `require_ready` must be exact booleans before ready-state policy can be
  selected.
  Strict release-bundle verifier release-checklist item diagnostics must number
  repeated generic item-shape blockers before public summary validation, so
  duplicated malformed copied checklist rows remain distinct without echoing
  row-local operator text.
  Strict release-bundle verifier source-inventory unknown-gate diagnostics must
  also number repeated generic malformed-name blockers before public summary
  validation, so duplicated copied hostile gate names remain distinct without
  leaking gate text.
  Strict release-bundle verifier corridor phase-key diagnostics must apply the
  same numbering rule to copied `phases` and `evidence_artifacts` maps, so
  duplicated malformed phase names cannot collapse or echo raw phase text.
  The scalar-text source inventory now also pins the bundle-builder and strict
  verifier repeated corridor phase-key regressions, the phase-key numbering
  helpers, and exact `#1`/`#2` assertions for hostile copied phase names.
  Strict release-bundle verifier all-lanes lane diagnostics must label
  malformed-domain rows by index, and readiness Markdown invariants must not
  require raw copied domain/chain text for invalid lane rows.
  Strict release-bundle verifier all-lanes source-adapter audit diagnostics must
  number repeated malformed audit-key and sensitive unexpected-audit blockers
  without leaking copied audit key text.
  Cryptographic-evidence audit-key regressions must also pin the existing
  row-numbering path for repeated malformed or sensitive
  `source_adapter_gate_audit_hashes` keys inside one copied crypto row.
  Release-bundle builder copied crypto rows and embedded/all-lanes
  `source_adapter_gate.audit_hashes` maps must apply the same numbering before
  Markdown rendering and avoid duplicate semantic re-emission of malformed
  audit-key shape diagnostics.
  The crypto-evidence binding source inventory now pins those bundle-builder,
  embedded/all-lanes, strict row, and strict audit-key repeated numbering
  regressions together with their numbering helpers and exact `#1`/`#2`
  assertions.
  Release-bundle builder copied public unknown-field diagnostics must number
  repeated redacted malformed or sensitive field-name blockers before Markdown
  rendering, while preserving safe public unknown-field names.
  Release-bundle builder copied source-inventory gate-name diagnostics must
  number repeated redacted malformed or sensitive gate-name blockers before
  Markdown rendering, while preserving safe public unknown-gate names.
  Strict release-bundle verifier source-inventory gate-name diagnostics must
  apply the same repeated-error numbering to unknown readiness gates, and the
  public JSON-root source inventory now pins the helper path, adversarial
  repeated-gate regression, redacted blocker payload, and exact `#1`/`#2`
  assertions.
  All current SCCP release/readiness `numbers_repeated` regressions must remain
  source-inventory pinned: public JSON-root summary/source-inventory repeats,
  input-provenance duplicate counters, release-checklist item errors,
  release-notes missing-artifact counters, and native-prover
  audit/SDK/artifact/blocker repeats now have direct marker coverage.
  Release-bundle builder copied corridor `phases` and `evidence_artifacts`
  phase-key diagnostics must number repeated redacted malformed or sensitive
  phase-name blockers before Markdown rendering, while preserving safe public
  unknown-phase names.
  Native EVM prover manifest and copied readiness-report `audit_hashes`
  diagnostics must number repeated redacted malformed or sensitive audit-key
  blockers before bundle-builder rendering or strict-summary validation,
  without leaking copied key text.


<a id="record-a91460e589fc4471215d1e8c7129910ebca14e26af2edf781bd865ba645cb36d"></a>

- SCCP release-bundle public blocker-list schemas must stay pinned as a
  readiness source-inventory gate: manifest, readiness-report, corridor,
  release-checklist, embedded evidence, standalone all-lanes root, lane,
  source-adapter-gate, and release-checklist blocker arrays, and active-launch
	  route-canary, route-allowlist, destination-rollout, and source-adapter-gate
	  blocker arrays must keep canonical non-empty strings, duplicate and
	  internal-space-normalized duplicate rejection, ready-surface empty-blocker
	  checks, and invalid-marker rendering for malformed blocker containers
	  before published bundle readiness can pass.
	  Duplicate blocker rejection must compare decoded, lowercased, and
	  ASCII-space-normalized public text, including HTML entity and bounded
	  URL-percent encoded forms, so encoded or repeated-space copies cannot
	  evade the repeated operator-text guard.
	  SCCP public blocker decoding now runs to a deterministic input-length-bounded
	  fixed point across the bundle builder, strict verifier, readiness renderer,
	  all-lanes summary, and lane evidence scripts; five-layer percent-encoded
	  Markdown markers and deeply nested HTML-entity sensitive names must still
	  collapse to fixed public blocker diagnostics before any copied operator text
	  is rendered.
	  Corridor `phases` and `evidence_artifacts` roots must also stay
	  object-shaped before phase-status, phase-artifact closure, and transcript
	  checks run, with hostile copied root values redacted from strict verifier
	  diagnostics.
	  The bundle builder must
	  reject malformed, empty, numeric, null, padded, or duplicate root blockers before
  `--allow-not-ready` diagnostics can render or write public artifacts.
  Sensitive blocker detection now also treats mnemonic, recovery-phrase,
  seed-phrase, credential, auth-header, and signing-key phrasing as fixed-category
  diagnostics in the bundle preflight, readiness renderer, and strict verifier,
  so not-ready diagnostics cannot echo common runtime signing material labels.
  The public blocker-list source inventory must pin those exact adversarial
  payload strings, not only the test names, so future edits cannot silently
  drift back to generic `secret-token` coverage.
  Readiness public blocker helper subclass boundaries must also stay pinned in
  that inventory so copied blocker list subclasses cannot be iterated before
  category-only diagnostics are produced.
  The source inventory now also pins shared repeated public blocker-list
  numbering regressions for redacted malformed strings and decoded duplicate
  groups, including the `#1`/`#2` fragments and encoded duplicate payloads.
  The same inventory now pins the deep percent-encoded and nested HTML-entity
  blocker regression, so bounded multi-pass decoding remains release-gated
  rather than only helper-implementation-gated.
  Readiness-report and strict bundle sparse inventory checks must remove every
  uniquely detectable public blocker-list marker across verifier schema checks,
  readiness wiring, bundle pre-render blocker checks, padded/duplicate/hostile
  blocker regressions, Markdown invalid-marker tests, native-prover blocker
  tests, and self-inventory rows.


<a id="record-cbfd76a22fb878e40fe615a3e141a3b4cace1c72e896fba184a3f1333578c81b"></a>

- SCCP release-bundle input provenance must stay pinned as a readiness
  source-inventory gate: copied evidence inputs must use canonical bundle paths,
  unique `inputs` and `input_artifacts`, the `evidence/NN-*.toml` layout, and
  verifier recomputation from copied TOML before published bundle readiness can
  pass. The bundle builder must also require and validate copied report
	  `inputs` before rendering release notes or writing public readiness artifacts,
	  rejecting empty/malformed input lists, duplicate input paths, escaped or
	  noncanonical paths, copied-layout drift, duplicate input artifact paths, and
	  `inputs`/`input_artifacts` mismatches. Padded copied paths and
	  direct or over-depth percent-encoded traversal in both `inputs` and
	  `input_artifacts` are pinned as pre-render and strict-verifier blockers
	  without raw path leakage. Strict bundle verification also pins backslash
	  and duplicate-separator copied provenance aliases before they can satisfy
	  public readiness. The standalone readiness-report
	  JSON renderer also rejects empty copied `inputs` and `input_artifacts` roots,
	  duplicate `inputs` paths, duplicate `input_artifacts` paths, and copied
	  `inputs`/`input_artifacts` path drift before public output can publish them.
	  Sparse inventory checks now
	  remove missing-input, malformed copied provenance, input path drift,
  provenance schema drift, report-artifact path drift, copied layout drift,
  no-usable-input, and secret path-redaction regressions directly. Readiness and
  strict-bundle sparse tests must remove every uniquely detectable input
  provenance marker across verifier schema checks, readiness wiring, bundle
  render preflights, copied-layout verification, adversarial bundle tests, and
  readiness self-inventory rows.


<a id="record-e48b3cbbd48f4e04aef6cc00b644eeca843e9eb7923bc87cdda6a863ae398e28"></a>

- SCCP release-bundle public JSON roots must stay pinned as a readiness
  source-inventory gate: manifest, readiness-report, and all-lanes JSON roots
  must keep canonical serialization, duplicate-key rejection, and non-UTF-8
  fail-closed diagnostics before published bundle readiness can pass. Strict
  verifier source-inventory read and UTF-8 decode failures must also stay
  category-only without echoing local source paths or OS/decoder exception
  payloads. The bundle
  builder must classify unknown readiness-report root field names and reject
  unknown readiness-report root fields before `--allow-not-ready` diagnostics
  can render or write public JSON artifacts; safe operator notes may remain
	  readable while padded, control-character, whitespace, Markdown-unsafe,
	  malformed, or Unicode-confusable root claims must be category-only blockers.
		  Standalone readiness-report JSON now also requires embedded all-lanes
		  `evidence` to be exactly canonical under the all-lanes public summary
		  sanitizer before publishing that root. Standalone corridor roots now also
		  require the exact public field set, canonical blockers, known phase keys,
		  allowed phase statuses, and valid artifact metadata before public output can
		  retain them. Standalone copied `native_evm_prover_bundle` roots now fail
		  closed unless their public field set, artifact metadata, audit hashes, SDK
		  artifact rows, validation status, and validation blockers satisfy the
		  native-prover summary schema. Standalone release-readiness public JSON also
			  rejects copied `release_checklist` roots whose success state contradicts their
				  blockers and copied `source_inventory` roots that are not object-shaped or
				  whose success state contradicts their blockers: ready checklist rows must have
				  empty blockers, a ready checklist root requires every item to be ready, passed
				  source-inventory gates must have empty validation blockers, and blocked gates
				  must carry at least one validation blocker. The same standalone public
			  readiness path must reject duplicate canonical blocker strings across root,
			  corridor, release-checklist, source-inventory, native-prover, and
			  user-prover blocker lists before copied public JSON or Markdown can publish
			  repeated operator text.
			  Active-launch release-checklist recomputation must apply the same duplicate
			  rejection to native EVM prover validation blockers before checklist or
			  Markdown output can copy operator-provided blocker text.
			  Readiness-report `source_inventory` gate names and known-gate row fields must
		  be schema-classified before unknown-gate or unknown-field diagnostics, so safe
		  operator notes remain readable while padded, control-character, whitespace,
		  Markdown-unsafe, sensitive, malformed, or Unicode-confusable source-inventory
		  claims are never echoed raw. Standalone readiness, release-bundle
		  pre-render, and strict verifier validation now walk copied
			  `source_inventory` gate and row-field keys as exact strings instead of
			  using hash/equality-based set membership, so hostile collision keys cannot
			  raise or leak before bounded malformed-gate/field diagnostics. Strict
			  all-lanes copied lane-schema checks now use the same exact string-key
			  traversal for required fields, active-lane selection, and domain
			  consistency, so hash-colliding lane keys cannot satisfy public release
			  readiness. Strict all-lanes lane-schema checks now also require exact
			  builtin containers before traversing copied lane rows, records, source
			  gates, audit hashes, EVM metadata, destination bindings, route
			  allowlists, route canaries, or cross-lane route-canary scanner inputs. The
			  standalone readiness-report JSON renderer
		  now also rejects syntactically safe unknown source-inventory gates and empty
		  or incomplete source-inventory maps before public output is emitted, because
		  every strict verifier required gate must be present. Empty copied
		  source-inventory maps also fail before release-bundle Markdown or public JSON
		  can be rendered for the same required-gate reason. Readiness must also reject
		  hostile verifier required-gate containers before trusting source-inventory
		  metadata, with a fixed invalid-gate diagnostic and no copied
		  set/stringification leakage. The bundle builder must also
		  classify copied source-inventory row field
	  names and reject malformed, padded, control-character, Markdown-unsafe,
  non-ASCII, or otherwise unknown copied source-inventory gate names, unknown row
  fields, non-passed validation status, and noncanonical or non-empty row blockers
	  before `--allow-not-ready` diagnostics can render or write public artifacts;
	  the source-inventory marker set must pin that copied blocker rejection
	  explicitly. Readiness-report and strict bundle sparse inventory checks must
		  also pin generated-bundle self-verification summaries, so malformed summary
		  roots, non-boolean `verified`, malformed verifier error lists, hostile
		  verifier error text, or noncanonical `manifest_sha256` values cannot escape
		  as tracebacking builder output or unchecked verified-root text.
		  Generated-bundle and verifier CLI strict-summary sanitizers now require
		  exact builtin summary roots, error lists, artifact lists, and artifact rows
		  before walking copied summary output. The standalone strict verifier CLI
		  must sanitize its own public summary before
	  JSON/text output and exit-code handling, including malformed internal roots,
	  unknown top-level summary fields, hostile error lists, malformed artifact
	  rows, and noncanonical `manifest_sha256` values. Sparse
	  Public helper-boundary regressions for release-bundle generator, strict
	  verifier, and readiness public-report container subclasses must also stay
	  source-inventory pinned so helper-level exact-container checks cannot drift
	  away from the published JSON readiness gate.
	  inventory checks must remove every uniquely detectable public JSON-root marker across duplicate-key,
	  canonical-serialization, UTF-8/JSON parsing, copied public-field schema,
	  source-inventory row-shape, adversarial bundle, redaction, and
  self-inventory guards.
  Standalone bundle verification must also reject readiness-report and
  all-lanes summary root subclasses, plus copied readiness-report evidence,
  corridor, native-prover, and source-inventory object-field subclasses, before
  schema traversal, artifact closure, or manifest-order recomputation can read
  hook-controlled containers. Late verifier checks must likewise reject copied
  readiness-report list subclasses and row subclasses for `inputs`,
  `input_artifacts`, `cryptographic_evidence`, and
  `user_prover_submission_surfaces` before artifact closure, crypto binding, or
  user-prover inventory traversal can iterate or read copied rows. Required
  source-inventory gate rows must also be exact builtin dictionaries before
  field reads, and blocker-empty checks must only truthiness-check exact builtin
  blocker lists.

