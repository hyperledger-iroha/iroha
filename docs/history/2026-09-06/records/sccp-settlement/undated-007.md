# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-684536cac6816cd3fa6fe226c8d5a473966dc63e992d057ecef50b8cccbc66dd"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP release-bundle public Markdown roots must stay pinned as a readiness
  source-inventory gate: readiness Markdown and release-note attachments must
  keep UTF-8 loading plus canonical text drift rejection before published
  bundle readiness can pass. Strict verifier and release-bundle builder
  load/render/recompute diagnostics for those public surfaces must remain
  category-only, including missing files, readiness Markdown rendering,
  release-notes attachment rendering, release-checklist recomputation, native
  prover summary recomputation, and user-prover submission-surface
  recomputation failures. The bundle builder must validate in-memory readiness
	  Markdown against the verifier-owned
	  invariants and canonical render before writing `sccp-release-readiness.md`,
	  so a weakened report renderer cannot publish drift before final bundle
	  verification. The verifier-owned canonical readiness Markdown renderer must
	  use only shallow exact-dictionary report copies and exact builtin corridor
	  phase/evidence-artifact maps before it reorders copied corridor metadata.
	  Readiness Markdown and release-notes attachment bundle wrappers, plus the
	  strict verifier invariant helpers, must also require exact builtin strings
	  before public text is split, checked for trailing newlines, or compared with
	  canonical renders, so string subclasses cannot execute hooks or leak copied
	  text through public diagnostics.
	  The source-inventory marker set must explicitly pin the bundle
  builder's pre-write readiness Markdown and release-notes attachment drift
	  rejections and the tests that assert no drifted public Markdown file is
	  written. Readiness-report and strict bundle sparse inventory checks must
	  remove every uniquely detectable public Markdown text marker across
	  verifier-side UTF-8/load/render/drift guards, bundle pre-write drift guards,
	  redaction tests, adversarial bundle tests, direct parent-symlink public
	  text-root tests, and readiness self-inventory rows.
	  Release-notes artifact tables now render zero-byte artifact rows as
	  `<invalid bytes>` and all-zero SHA-256 artifact rows as
	  `<invalid artifact.sha256>`, matching the strict manifest/readiness
	  positive-byte and non-zero-hash artifact contract before copied metadata can
	  look valid in public notes.
	  Release-notes attachment artifact rendering and invariant checks must require
	  copied artifact roots and rows to be exact builtin containers before path,
	  byte, hash, self-row, or manifest-root checks can read copied metadata.
	  Release-notes attachment rendering and invariant checks must require copied
	  report roots to be exact builtin dictionaries before status or blocker fields
	  can influence public attachment text.
  Readiness Markdown
	  source-inventory blocker checks must suppress malformed source-inventory gate
  names before emitting secondary missing-cell diagnostics, and copied
  input/corridor report-artifact paths must pass path classification before
  Markdown path/hash presence checks.
  Input/corridor Markdown invariant checks must require copied
  `input_artifacts` roots and rows, corridor roots, corridor phase maps,
  corridor evidence-artifact maps, and per-phase artifact rows to be exact
  builtin containers before row presence checks can recompute or iterate copied
  data.
  Strict readiness Markdown invariant checks must also require copied
  `source_inventory` roots and gate rows to be exact builtin dictionaries, and
  blocker lists to be exact builtin lists, before public Markdown row presence
  checks can recompute or iterate copied data.
  The same invariant path must require copied `native_evm_prover_bundle` roots
  to be exact builtin dictionaries, and native validation-blocker lists to be
  exact builtin lists, before native prover Markdown row checks can recompute or
  iterate copied data.
  User-prover Markdown invariant checks must likewise require copied
  submission-surface roots and rows to be exact builtin containers, and helper,
  required-phase, and validation-blocker lists to be exact builtin lists, before
  row presence checks can recompute or iterate copied data.
  Lane-readiness Markdown invariant checks must require copied evidence roots,
  lane lists, and lane rows to be exact builtin containers, and lane blocker
  lists to be exact builtin lists, before lane row presence checks can recompute
  or iterate copied data.
  Cryptographic-evidence Markdown invariant checks must require copied
  `cryptographic_evidence` roots and rows to be exact builtin containers, and
  source-adapter audit maps to be exact builtin dictionaries, before crypto row
  presence checks can recompute or iterate copied data.
  Cryptographic-evidence row domains and source-adapter audit keys must also be
  schema-classified before stale Markdown presence checks can mention row or
  audit labels.


<a id="record-fd65488920cc3161a9beb1dc55d591c8baae9be323bd04135b47fe9c8017daf9"></a>

- SCCP release-bundle public cryptographic-evidence binding must stay pinned as
  a readiness source-inventory gate: production-domain row inventory,
  lane-field binding, canonical row recomputation, and active route-canary
  binding rejection must remain required before published bundle readiness can
  pass. Active-row `source_adapter_gate_audit_hashes` keys must be
  schema-classified before semantic hash or unexpected-field checks so control
  characters, whitespace, Markdown-unsafe characters, and Unicode confusables
  cannot leak through raw public diagnostics. Malformed cryptographic-evidence
  row domains and audit keys must also be suppressed before Markdown
  row/audit-presence diagnostics. Hostile copied audit-key aliases generated
  during expected-row recomputation must stay pinned on both readiness and
  strict-verifier paths so object stringification cannot enter public blockers.
  Hostile nested required-key aliases for source-record, destination-binding,
  route-allowlist, route-canary, source-adapter gate, and live-metadata rows
  must also be pinned so expected-row recomputation only consumes exact builtin
  public keys.
  The bundle builder must reject unknown or
  malformed cryptographic-evidence row fields, row scalars, audit-hash entries,
  boolean/null drift, optional bytes32 and block-number drift, duplicate
  domains, row/lane count drift, and copied row-to-embedded-lane
  binding drift before `--allow-not-ready` diagnostics can render or write
	  public artifacts. EVM route-canary public rows must also require exact
	  `evm_message_proof_accepted_transaction` evidence source plus exact `true`
	  evidence-bound, message-proof-used, and finalized-receipt booleans, and
	  positive u32 receipt block numbers, before public Markdown can be rendered or
	  strict bundle verification can pass. The release-bundle builder must classify
	  BSC EVM route-canary rows through the typed `_sccp_domain_bsc()` helper,
	  with a regression that temporarily moves the BSC domain id before validating
	  message-proof and finalized-receipt metadata, so a stale literal domain cannot
	  drop BSC out of public crypto validation. Standalone readiness-report public
	  crypto source-gate policy maps must also key BSC by `SCCP_DOMAIN_BSC`, with a
	  direct source-text regression and strict inventory markers so literal domain
	  policy wiring cannot return. TRON route-canary public rows must also
	  expose exact `true` `route_canary_message_proof_used` for message-proof
  route-canary evidence and exact `true`
  `route_canary_raw_data_owner_matches_transaction` plus
  `route_canary_signature_recovers_to_owner` for TRON transaction-owner and
  signature-recovery evidence, while Solana/TON snapshot canary rows and EVM
  rows must keep those TRON-only public fields `null`; standalone
  readiness-report negatives now exercise that non-TRON null policy across
  every non-TRON launch-domain row. Copied rows with
  missing, false, string, or lane-drifted message-proof, owner-binding, or
  signature-recovery values now fail before public output is emitted. TRON
  route-canary public rows must also keep block numbers as positive u64
  integers and block timestamps as non-negative u64 integers before
  that same public-output boundary. Public source-adapter gate rows must also
	  enforce domain-specific audit-key policy for Solana, TON, and TRON rows before
	  non-active copied evidence can pass. The standalone readiness-report JSON
	  renderer now rejects empty or incomplete copied `cryptographic_evidence` rows
	  before public output is emitted by requiring every launch-domain row exactly
	  once, and it rejects contradictory copied source-adapter gate semantics before
	  JSON or Markdown can publish a row whose required flag, expected audit-key set,
	  gate hash, or audit hash no longer matches the launch-domain gate policy.
	  Standalone readiness-report negatives now exercise missing expected audit
	  keys, unexpected audit keys, and gate-hash/audit mismatch across every
	  launch-domain row.
	  Public cryptographic-evidence rows must also keep route-canary evidence
	  hashes distinct from copied route-canary transcript hashes, so a
	  message id, receipt block hash, receipts root, or transaction hash cannot
	  be republished as the evidence commitment.
	  Readiness-report and strict bundle sparse
	  inventory checks must remove every uniquely detectable cryptographic-evidence
  binding marker across verifier schema checks, readiness wiring, bundle row
  schema checks, route-canary metadata, source-adapter gate policy, adversarial
  bundle tests, redaction tests, BSC/TRON profile checks, and self-inventory
  rows.
  Strict verifier cryptographic-evidence row diagnostics must also number
  repeated generic row blockers before public summary validation, so duplicate
  copied crypto row shape failures remain distinct without leaking row-local
  operator text.
  The public cryptographic-evidence binding inventory now also has explicit
  active-lane coverage sentinels for route-canary evidence source and
  source-adapter gate audit policy across `eth`, `bsc`, `sol`, `ton`, and
  `tron`, with readiness/strict-bundle negative tests that remove a lane's
  sentinels before the remaining crypto-evidence marker set can pass.


<a id="record-e8c5abf3bb1b0da9c387ed39e9820ad979650924c5c312f2d3eb83bc951defc2"></a>

- SCCP release-bundle public submission-surface binding must stay pinned as a
  readiness source-inventory gate: lane/backend inventory, per-SDK helper
  inventory, verifier-owned surface recomputation, and corridor-phase binding
  must remain required before published bundle readiness can pass. Public
  `user_prover_submission_surfaces` roots must remain non-empty lists before
  recomputation or Markdown checks, and hostile scalar roots must be reduced to
  redacted shape diagnostics instead of raw operator text. Public
  `user_prover_submission_surfaces[].lanes` labels must be schema-classified
  before lane inventory, backend, helper, or Markdown-presence checks, so
  padded, control-character, whitespace, Markdown-unsafe, malformed, or
  non-ASCII/confusable lane labels cannot leak raw public diagnostics.
  `user_prover_submission_surfaces[].proof_backend` values must use the same
  classification before backend mismatch or Markdown-presence checks, preserving
  readable safe unknown backend diagnostics while suppressing hostile backend
  ids.
  `user_prover_submission_surfaces[].on_chain_submission` text must match the
  verifier-owned lane submission text before Markdown-presence checks, so copied
  operator text or hostile submission labels cannot leak raw public diagnostics.
  Readiness, bundle, and strict-verifier submission-surface checks must require
  exact builtin strings for lane/backend/submission text, helper summaries,
  helper symbols, required phases, validation status, and public mapping keys
  before equality, membership, or Markdown rendering can run, so string
  subclasses cannot satisfy or crash public row binding.
  Helper-set hostile SDK alias and container-subclass regressions must stay
  pinned in the same source inventory so copied helper maps cannot satisfy
  required user-prover SDK coverage by key aliasing.
  Default and per-SDK helper symbols must be schema-classified before helper
  string derivation, helper inventory, UI-hook matching, or Markdown-presence
  checks, so table-breaking or confusable helper names become category-only
  blockers. Copied top-level JS helper lists and per-SDK helper maps must also
  match the verifier-owned expected helper inventories exactly, not merely
  contain the required helpers, so safe-looking extra helper claims cannot
  over-advertise portal/mobile submission coverage before public output is
  rendered or verified. If a copied report corrupts the per-SDK helper map or
  any helper entry, readiness Markdown must render an invalid-marker cell
  instead of falling back to raw `sdk_helpers` text or rendering the raw helper.
  Public `sdk_helper_symbols_by_sdk` map keys must be schema-classified before
  unknown-SDK, helper-list, or Markdown-presence diagnostics, so malformed
  padded, control-character, whitespace, Markdown-unsafe, and
  non-ASCII/confusable SDK keys cannot leak raw public diagnostics.
  `required_phases` values use the same classification before unknown-phase,
  duplicate, missing-phase, contract-smoke, or Markdown-presence checks, while
  canonical safe unknown phases remain readable operator diagnostics. The strict
  verifier and bundle builder must also compare required phases to the
  verifier-owned lane policy exactly, so reordered or extra known phases such as
  a non-EVM `dotnet-sdk` cannot satisfy public portal/mobile rows through a
  generic row recomputation check. The strict verifier must also number repeated
  generic user-prover row diagnostics before public summary validation, so
  duplicate copied row shape failures remain distinct without leaking row-local
  operator text. The source inventory now also pins repeated builder-side
  SDK-key, helper-symbol, validation-blocker, required-phase, shared
  validation-blocker duplicate, and strict row-error numbering regressions with
  their `#1`/`#2` assertions and adversarial payloads. The bundle builder must
  reject unknown or
  malformed submission-surface row fields,
  scalar text, helper-symbol lists, per-SDK helper maps, SDK keys, required
  phase values, validation status, and validation blockers before
  `--allow-not-ready` diagnostics can render or write public artifacts.
  Copied submission rows with `validation_status = blocked` or non-empty
  validation blockers are now rejected directly before Markdown or JSON output is
  written, even when the row shape is otherwise canonical. Submission-surface
  validation blockers must also compare decoded/lowercased public text for
  duplicate rejection and apply the same recovery-phrase sensitive-name policy,
  so HTML entity or bounded URL-percent encoded copies cannot repeat operator
  blocker text or publish recovery-phrase labels. Readiness-report and
  strict bundle sparse inventory checks must remove every uniquely detectable
  submission-surface binding marker across verifier recomputation, readiness
  rendering, bundle row schema checks, validation-status/blocker coupling,
  copied-corridor phase binding, helper inventory, adversarial bundle tests,
  redaction tests, and self-inventory rows.
  The public submission-surface binding inventory now also pins exact
  lane/backend rows plus representative SDK helper sentinels for `eth`, `bsc`,
  `sol`, `ton`, and `tron`, with readiness/strict-bundle negative tests that
  remove a lane's sentinel before the remaining submission-surface marker set
  can pass.
  Unknown submission-surface row fields use the same structured field-name
  classification in the verifier and release-bundle builder before render, so
  valid operator notes stay readable while padded, control-character,
  whitespace, Markdown-unsafe, malformed, or Unicode-confusable field names
  never leak raw public diagnostics. Copied submission-surface rows must also
  recompute from copied corridor phases and reject duplicate/unknown/missing lane
  rows, backend drift, missing required SDK helpers, and blocked validation rows
  before public output is written.


<a id="record-3fb8779ef60dcfeb597b5b3d2b8c46d766311a679e881e6d71a138e7347d6013"></a>

- SCCP native EVM prover validation blockers must stay schema-aware in both
  readiness generation and release-bundle verification: scalar blocker
  containers, non-string entries, empty strings, and padded strings must become
  explicit readiness blockers rather than being filtered, character-expanded, or
  silently treated as ready. Sparse inventory tests must pin the malformed
  native-blocker regressions and no-character-expansion assertions in both
  readiness generation and strict bundle verification.
  Copied release-bundle native prover summaries with `validation_status =
  blocked` or non-empty validation blockers are now rejected directly before
  public Markdown or JSON output is written, even when the summary shape is
	  otherwise minimal and canonical; sparse inventory tests must keep the blocked
	  copied-summary pre-render regression and blocker marker pinned, and strict
	  bundle inventory now removes that marker directly to prove the gate fails.
	  Standalone public readiness JSON also pins unexplained blocked native-prover
	  and user-prover roots, suppressing copied roots and operator notes when
	  blocked validation rows omit canonical blockers.
	  Active launch checklist recomputation now also rejects a blocked native EVM
	  prover status with an empty `validation_blockers` list before report-root
	  sanitization, and both standalone readiness and strict bundle tests pin the
	  helper-level diagnostic in source inventory.
	  Copied `validation_status` values now require exact strings before native
	  prover, source-inventory, or user-prover readiness decisions in standalone
	  public JSON, bundle pre-render validation, and strict bundle verification;
	  Markdown status/blocker cells follow the same rule, and hostile string
	  subclasses are covered by adversarial tests and cannot trigger equality hooks
	  or set-membership hooks.
	  Native EVM prover SDK artifact ids must follow the same canonical text policy:
	  whitespace-padded, control-character, internal-whitespace,
  non-ASCII/confusable, and malformed lowercase-id SDK ids are rejected as
  malformed rows instead of being treated as unknown SDK names or hidden
  missing-SDK evidence.
  Repeated sensitive native SDK names must also stay distinct without leaking
  raw names: readiness generation, release-bundle pre-render checks, and strict
  bundle verification number repeated redacted SDK-name blockers for native SDK
  artifact rows and parity/self-test `sdk_results` rows before publishing
  native EVM prover validation diagnostics.
  Published readiness-report `native_evm_prover_bundle.sdk_artifacts[].sdk`
  rows enforce the same canonical SDK-id policy so tampered report JSON cannot
  downgrade malformed ids into raw unknown-SDK diagnostics.
  SDK artifact row schemas also reject malformed unknown field names with the
  same structured policy in readiness generation, bundled native-manifest
  verification, and published readiness-report summary verification.
  Native prover parity and self-test fixture `sdk_results` keys follow the same
  policy in readiness generation and strict bundle verification, so padded SDK
  result keys surface as schema blockers before fixture rows can be treated as
  unknown or missing SDK evidence.
  Control-character, internal-whitespace, non-ASCII/confusable, and malformed
  lowercase-id SDK result keys are also rejected as malformed keys before
  unknown-SDK classification, keeping public diagnostics structured and
  release-bundle fixture reviews fail-closed. Repeated sensitive unknown
  `sdk_results` keys are numbered in the same redacted diagnostic style as
  native SDK artifact rows.
  Native EVM prover bundle generators must also keep route/deployment JSON and
  every cryptographic or SDK artifact input regular-file-only, with symlinks and
  out-of-root realpaths rejected before hashing or attaching production route
  manifest material. Release builder, readiness-report, and verifier path-text
  gates now reject raw, percent-encoded, and recursively over-encoded
  parent-directory segments before generated artifacts, manifest rows,
  report-provenance paths, extracted bundle entries, or native prover payload
  paths are published for browser runtime consumption; keep the inventory gate
  pinned as regression coverage, including sparse checks for percent-encoded
  native payload regressions in readiness generation and strict bundle
  verification. Native prover payload artifact path metadata
  failures now render fixed blockers in readiness generation and strict bundle
  verification, so local path-validation details cannot leak into published
  native prover diagnostics.
  Native prover artifact helper output must also be shape-checked before any
  byte/hash indexing: non-object metadata, unsafe metadata paths, boolean or
  non-integer byte counts, and noncanonical SHA-256 text remain category-only
  blockers in both readiness generation and strict release-bundle verification.
  Published native prover SDK implementation-artifact metadata diagnostics must
  stay numbered for duplicate SDK rows so repeated unknown-field, byte-count,
  and SHA-256 blockers cannot collapse into duplicate strict-summary errors.
  Portal/mobile runtime SDK selectors for direct byte verification,
  resolver-backed bundle loading, and native prover self-test preflights must
  follow the same canonical text policy before SDK artifact lookup or callbacks
  run. Native no-WASM/no-remote source inventory must pin those padded-SDK
  negative tests across JavaScript, Kotlin/JVM, Java Android, and Swift before
  native prover evidence can pass, and sparse inventory self-tests must cover the
  Kotlin/JVM and Java Android padded self-test callback non-run markers directly.
  Sparse inventory checks must also remove the browser no-WASM guard, BSC
  browser guard, URI proof-artifact, WASM proof-artifact, and remote-prover
  identifier markers from the JavaScript package distribution test directly.


<a id="record-1114fe92a945fa59de87d21812f5619c64954787089eeeded32af93019abacd3"></a>

- SCCP readiness cryptographic-evidence rows must preserve raw boolean and
  container values from the normalized lane summary. Truthy strings, numeric
  gate hashes, or malformed source-gate audit containers must remain visible to
  release-bundle schema checks instead of being coerced into ready-looking
  values. Missing future-lane route-canary bindings must render as explicit
  boolean `false`, while present malformed binding values remain preserved for
  verifier rejection. Published `cryptographic_evidence` roots must also stay
  non-empty lists before inventory, lane-binding, or embedded-evidence
  recomputation, and hostile scalar roots must reduce to fixed missing-root
  diagnostics without echoing operator text. Release-bundle copied
  `cryptographic_evidence` row validation must select route-canary and
  source-gate policy by exact integer domains only, so boolean domains cannot
  alias ETH/EVM route-canary or source-gate requirements after the malformed
  domain diagnostic is emitted. Source inventory must sparse-check the
  readiness-side malformed audit-container preservation assertion before public
  cryptographic-evidence readiness can pass.

