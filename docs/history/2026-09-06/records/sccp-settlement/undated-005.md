# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-a13fa55cbbefe9998029984df236cc1f8d1b63f87b3ee867ed94405b5e92d3f1"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP release-bundle corridor schema must classify unknown corridor root
  fields and corridor `phases`/`evidence_artifacts` keys before semantic phase
  lookup, manifest artifact ownership, transcript inspection, or Markdown
  invariant checks. Safe ASCII operator names may remain readable in
  diagnostics, but padded, control-character, whitespace, Markdown-unsafe,
  malformed, or Unicode-confusable keys must be category-only blockers. The
  Required Release Evidence invariant must keep the release corridor
  phase-transcript source-inventory row before public bundle readiness can pass.
  Release-bundle preflight must stage external phase evidence under a temporary
  bundle root and hand readiness only `corridor/<phase>.log` public paths, so
  absolute operator source paths never weaken the standalone artifact boundary.
	  bundle builder must also require the canonical corridor root shape,
	  classify malformed copied corridor root fields before render, require
	  canonical corridor blocker lists, reject malformed copied phase-map keys,
	  reject invalid copied phase statuses, and require hashed evidence artifacts
	  for copied passed phases before `--allow-not-ready` diagnostics can render or
	  write public artifacts. Copied corridor summaries must also keep
	  `production_ready = true` and `require_phase_evidence = true` before public
	  output is written, matching the strict verifier's readiness requirements.
	  Empty copied corridor maps now run the same required-field checks before
	  release-bundle Markdown or public JSON can be rendered.


<a id="record-24e3e73015a2c220f81c169116615c1e1a34a3b8d1fe8febfe2dae1a52e72c2d"></a>

- SCCP all-lanes public JSON schemas must classify unknown summary, lane,
  nested evidence-object, route-canary, and source-adapter audit-hash keys
  before semantic matching or hash-role checks. Safe ASCII operator names may
  remain readable, but malformed or Unicode-confusable keys must never be
  echoed from readiness-report embedded evidence or the standalone all-lanes
	  summary. The bundle builder must reject copied embedded evidence root/lane
	  unknown fields, malformed domain/chain/production-ready scalars, noncanonical
	  blocker lists, malformed record-flag containers, and nested evidence-object
	  shape drift before `--allow-not-ready` diagnostics can render or write public
	  artifacts. Copied embedded evidence must also recompute from the copied TOML
	  evidence input artifacts before rendering, so root-level summary drift cannot
	  publish before the strict bundle verifier compares the final JSON files.
		  The strict verifier must also reject duplicate integer entries in copied
		  all-lanes domain lists, including `supported_launch_domains` and
		  `unsupported_launch_domains`, before relying only on launch-scope set
		  comparison diagnostics. The bundle builder's pre-render copied-evidence
		  schema regression now pins duplicate `required_domains`,
		  `supported_launch_domains`, and `unsupported_launch_domains` blockers
		  before public artifacts are written. The bundle builder must also reject
		  copied launch-domain scope drift before rendering, including non-exact
		  `supported_launch_domains`, non-empty or non-exact
		  `unsupported_launch_domains`, overlapping supported/unsupported lists,
		  and supported-plus-unsupported lists that no longer match
		  `required_domains`. Copied all-lanes summary, lane, and `records`
		  unknown field names must use the same public-field classifier before
		  bundle rendering, so padded, control-character, whitespace,
	  Markdown-unsafe, or Unicode-confusable names cannot leak raw public
	  diagnostics. The copied-evidence pre-render regressions now also inject
	  non-string root, lane, and `records` keys and require bounded malformed-name
	  blockers before Markdown or JSON artifacts can be written.
	  Copied embedded evidence must also classify nested
	  `source_record_hashes`, `source_adapter_gate.audit_hashes`,
  `evm_live_metadata`, `destination_binding`, `route_allowlist`, and
  `route_allowlist.route_canary` field/key/hash/scalar drift before public
  diagnostics can be rendered. Ready copied summaries must additionally keep
  source-record, destination-binding, route-allowlist, and route-canary evidence
  hashes as canonical non-zero bytes32 values, and must preserve route-canary
  status/source, evidence-bound booleans, lane-domain/proof constants,
  transcript counters, TRON signer binding, Solana ProgramData slots, and TON
  last-transaction LT in their canonical public-summary shapes before public
  artifacts are emitted. The nested-map pre-render regression now also injects non-string
  copied keys into each of those nested maps and pins malformed
  unknown/audit-field blockers in the all-lanes evidence-root source inventory.
	  Active EVM copied metadata must also keep
	  `required = true`, active readiness, canonical decimal source/destination
	  chain ids, Ethereum `finalized` block tags, and BSC `latest` block tags
	  before not-ready diagnostics can be emitted. The active launch lane also cannot bypass source-record
  template rejection in standalone all-lanes output or pre-render bundle
  validation by marking the copied lane summary not-ready. No active
  source-record hash field may disappear from standalone copied summaries or
  pre-render bundle validation even when the copied active lane is marked
  not-ready. Copied all-lanes release checklists must also keep their root
  `ready` and `items` fields; an empty copied checklist map is rejected before
  standalone output or release-bundle Markdown can be rendered. Copied all-lanes
  root fields are now required even for diagnostic not-ready summaries, so
  required domain lists, lanes, production readiness, and release checklist roots
  cannot disappear before standalone output or bundle rendering fails. Future diagnostic
  lanes may stay incomplete during the
  first-launch bundle, but any copied nested fields they do include must still
  pass canonical shape checks; the standalone all-lanes public-summary
  preflight now rejects malformed present source-record, source-gate,
  destination-binding, route-allowlist, and route-canary hash fields even when a
  copied summary is already not-ready. It also rejects malformed present EVM
  live metadata, destination-binding, route-allowlist, and route-canary scalar
  fields before not-ready public JSON can preserve copied operator text.
  Canonical-but-wrong copied EVM live metadata, destination/route hash-match
  flags, route-canary status/source/boolean/domain/proof constants, and TRON
  signer bindings are rejected at the same not-ready public-summary boundary.
  Copied lanes that claim `production_ready = true` now always run the strict
  ready-lane checks, even when the aggregate summary is already not-ready, and
  standalone copied summaries enforce the same destination-family field rules
  for claimed-ready EVM, Solana, TON, and TRON lanes plus exact destination,
	  route, and route-canary sibling hash bindings; claimed-ready EVM live metadata
	  and destination binding keys are now required before copied metadata is
	  accepted, and non-EVM launch lanes must keep copied EVM live metadata in the
	  strict empty/non-required state before public summaries can retain it.
	  Copied readiness input-artifact rows must also keep positive byte counts
	  before public JSON can be rendered, matching the strict release-bundle
	  verifier.
	  Standalone copied route-canary records now use the strict domain-specific
	  field sets, so EVM, Solana, TON, and TRON canary fields cannot be replayed
  into the wrong launch lane before public summaries are rendered, and omitted
  domain-specific route-canary fields now fail with bounded missing-field
  blockers even for diagnostic not-ready lanes. Other fixed nested copied lane
  objects now do the same for omitted source-record hash, source-adapter
  gate/audit, EVM live metadata, destination-binding, and route-allowlist fields
  before public summaries are rendered. Copied EVM live metadata and
  destination-family fields also reject empty required values and missing
  family-specific destination fields on diagnostic not-ready lanes, matching
  the strict release verifier. Copied source-adapter gates now run the same
  required/ready/blocker, empty not-required material, gate-to-audit, and
  hash-role semantics for diagnostic not-ready lanes before public summaries
  are rendered. Copied source-record, destination-binding, route-allowlist, and
  route-canary commitment hashes must also remain canonical non-zero bytes32
  values on diagnostic not-ready lanes. The active Ethereum launch lane is
  ready-required even inside not-ready aggregate diagnostics, so copied
  active-lane `production_ready`, record flags, source-gate readiness, and
  blockers cannot be downgraded before public summaries are rendered. Active
  record flags also cannot disappear through an empty copied `records` map
  before standalone output or pre-render bundle validation fails, and active
  lane-root fields cannot disappear before the same pre-render boundary. Copied
	  active-lane EVM live metadata must also keep required/ready flags, canonical
	  Ethereum/BSC chain IDs, Ethereum `finalized` tags, and BSC `latest` tags
	  even when the copied lane summary is marked not-ready, and copied active-lane destination-family metadata must
  retain the EVM destination network id plus canonical bridge address in both
  standalone all-lanes output and pre-render bundle validation.
  No active destination-binding core field or required EVM-family destination
  field may disappear from standalone copied summaries or pre-render bundle
  validation even when the copied active lane is marked not-ready.
	  No active EVM live metadata field may disappear from standalone copied
	  summaries or pre-render bundle validation even when the copied active lane is
	  marked not-ready.
	  Readiness and strict-verifier active EVM live metadata blockers now require
	  exact builtin metadata maps before reading source/destination RPC chain IDs or
	  block tags, so hostile copied metadata subclasses are classified as malformed
	  without executing hooks.
	  All-lanes optional copied canonical-string helpers must require exact boolean
	  `allow_empty` controls before empty diagnostic lane metadata can be accepted.
	  Destination, route-allowlist, and route-canary sibling hash bindings must
	  also stay exact when copied lanes are diagnostic and not-ready, so true-looking
  hash-match flags cannot preserve contradictory hashes in public summaries.
  Copied route-canary hash roles must also stay distinct from source-record,
  source-gate, destination, route, and lane-specific route-canary transcript
  roles on diagnostic not-ready lanes before public summaries are rendered.
  Copied route-allowlist hashes must recompute from the copied source material,
  source-adapter deployment, and destination-binding hashes even when a
  diagnostic lane marks the expected route hash as matched. Copied
  route-allowlist recomputation helper failures now fail closed with the same
  fixed recompute blocker, so helper exceptions cannot silently preserve a
  forged copied route hash or leak parser details into public JSON. Strict
  bundle verification now also bounds canonical fixed-hex byte parser failures
  inside template-replay and route-allowlist recompute checks, emitting
  canonical bytes32 blockers instead of leaking parser detail or skipping
  replay and recompute validation. No active
  route-allowlist field may disappear from standalone copied summaries or
  pre-render bundle validation even when the copied active lane is marked
  not-ready. Copied
  destination-binding hashes must also recompute from the copied canonical
  destination-binding key before either standalone all-lanes output or release
  bundle verification can accept a self-consistent copied hash pair. The
  release-bundle builder now applies the same pre-render destination-binding
  recompute requirement and the source/deployment/destination route-allowlist
  recompute requirement to copied active and production-ready lanes, including
  production-ready non-active lanes, before public bundle artifacts can be
  written; the active launch lane cannot bypass either recompute boundary by
  marking its copied standalone or bundled lane summary not-ready. The same
  active-lane not-ready bypass is now pinned for copied route-canary hash-role
  separation, including EVM `message_id` source-material replay, in standalone
  all-lanes output and pre-render bundle validation.
  Copied source-adapter gates for active or
  production-ready lanes must also preserve exact boolean `required`/`ready`
  flags, domain-specific required/empty gate policy, expected audit-key sets,
  gate-hash-to-audit matching, and empty ready-gate blockers before public
  bundle output is written; the active launch lane cannot bypass those
  source-gate checks by marking its copied bundled lane summary not-ready.
  No active source-adapter gate field may disappear from standalone copied
  summaries or pre-render bundle validation even when the copied active lane is
  marked not-ready.
  Copied route-canary records
  for those lanes must preserve common semantic bindings as well: `status =
  passed`, expected lane evidence source, `evidence_bound = true`, and
  route/destination hashes matching the sibling lane records before Markdown or
  public JSON output is written; the active launch lane cannot bypass those
  route-canary semantic checks by marking its copied standalone or bundled lane
  summary not-ready. Direct all-lanes release-checklist validation must also
  require copied route-canary `route_allowlist_hash` and
  `destination_binding_hash` fields to be canonical non-zero bytes32 values
  that match the sibling route and destination summaries before live-canary or
  no-unresolved checklist items can pass, so `evidence_bound = true` cannot
  hide missing, zero, malformed, or drifted canary bindings. The same direct
  checklist path must reject copied route-canary `evidence_hash` values that
  replay the same lane's source-record hashes, destination binding hash, route
  allowlist hash, source-adapter gate hash, or source-adapter gate audit hashes
  before live-canary or no-unresolved checklist items can pass. Direct
  checklist validation must also keep copied route-canary proof-context scalars
  lane-specific: EVM receipt/log/proof constants, TRON block/log/proof
  constants plus owner/recovered-owner addresses, Solana ProgramData
  address/slot, and TON last-transaction LT must all keep canonical production
  shapes before live-canary or no-unresolved checklist items can pass; TRON
  addresses must stay non-zero canonical `0x41`-prefixed 21-byte hex. Direct
  checklist validation must also require copied EVM, TRON, and TON route-canary
  transcript hashes to remain canonical non-zero bytes32 values and
  role-separated from governed lane hashes and sibling transcript hashes before
  live-canary or no-unresolved checklist items can pass. The same direct
  checklist path must reject built-in source-material template hashes copied
  into route-canary evidence or transcript fields, with template-loader
  failures collapsed to fixed checklist blockers that do not leak helper
  exception text. Direct checklist validation must also require copied
  destination-binding keys, destination recomputed flags, and destination
  hashes to be canonical and internally recomputed, and copied route-allowlist
  hashes must recompute from copied source material, source-adapter deployment,
  and destination hashes, with recompute helper failures collapsed to fixed
  checklist blockers before governed-deployment, route, or no-unresolved items
  can pass. Direct top-level lane blockers must keep category boundaries and
  redaction exact: valid non-route lane blockers only hold the no-unresolved
  gate, route-canary lane blockers also hold live-canary readiness, and encoded
  sensitive/control/Markdown-unsafe or duplicate blocker text collapses to
  fixed diagnostics without copied operator text. Route-canary lane-blocker
  classification must use the same decoded, casefolded public blocker key as
  duplicate detection, so safe case variants or encoded spaces cannot bypass
  live-canary readiness while unsafe decoded blockers still collapse to fixed
  diagnostics. Deployment and route-allowlist lane-blocker classification must
  use the same decoded, casefolded public blocker key so safe case variants or
  encoded spaces cannot bypass governed-deployment or route readiness. Direct
  root preflight blockers must also be canonicalized before
  seeding no-unresolved readiness, so scalar roots, non-string entries, encoded
  sensitive/control/Markdown-unsafe text, and decoded duplicate root blockers
  become fixed diagnostics instead of leaking copied root text. Direct checklist
  lane labels must only include
  copied `chain` text when it is one of the known SCCP launch-chain spellings;
  malformed, unsupported, or hostile chain strings must fall back to bounded
  `lane`/`domain N` labels before metadata, canary, or unresolved blockers are
  rendered. Direct source-record validation must also reject copied source
  adapter deployment hashes that reuse the copied source verifier material hash,
  so canonical non-zero but role-replayed source records cannot satisfy source
  or no-unresolved readiness. Direct governed-deployment validation must also
  reject destination binding hashes that replay copied source verifier material
  or source adapter deployment hashes, so self-consistent destination hash pairs
  cannot satisfy governed-deployment or no-unresolved readiness when they reuse
  source roles. Direct route-allowlist validation must also reject route hashes
  that replay copied source verifier material, source adapter deployment, or
  destination binding hashes, so self-consistent route hash pairs cannot satisfy
  route or no-unresolved readiness when they reuse governed roles. Direct
  route-canary validation must also emit the exact unbound-evidence blocker
  whenever `evidence_bound` is not `true`, even when copied top-level lane
  blockers already mention route-canary work, so copied blockers cannot mask the
  evidence-bound failure. Active EVM
  route-canary proof metadata now likewise keeps
  target domain, proof version, proof source domain, message-proof usage, and
  finalized receipt state exact in standalone copied summaries and pre-render
  bundle validation even when the copied active lane is marked not-ready, with
  source-inventory markers pinning the direct readiness and strict-verifier
  checklist helpers.
  Active EVM route-canary transcript hashes now also remain canonical,
  non-zero, and role-separated from other transcript hashes and governed lane
  hashes in standalone copied summaries and pre-render bundle validation even
  when the copied active lane is marked not-ready. The direct active checklist
  covers the full EVM transcript set: call-data SHA-256, payload hash,
  statement hash, commitment root, finality height, finality block hash,
  transaction hash, receipt block hash, receipts root, and message id.
  Active EVM route-canary scalar metadata now also keeps `log_index` within
  u32 bounds and receipt block numbers positive in standalone copied summaries
  and pre-render bundle validation even when the copied active lane is marked
  not-ready, with direct readiness and strict-verifier checklist coverage for
  string, boolean, negative, overflow, and missing `log_index` drift.
  Direct active route-canary checklist validation now also requires copied
  canary route-allowlist and destination-binding hashes to remain canonical
  non-zero bytes32 values that match the lane hashes, while pre-render
  validation still rejects any missing common, scalar, transcript, or proof
  metadata even when the copied active lane is marked not-ready.
  Copied route-canary evidence hashes must also stay
  distinct from same-lane governed hashes, same-lane canary roles, other lane
  canary evidence hashes, other lane route-canary transcript hashes, and other
  lane governed hashes, including source-adapter gate hashes and audit hashes,
  before public output is written. Standalone readiness-report public
  cryptographic-evidence negatives now exercise same-row route-canary transcript
  replay rejection across every launch-domain row too.
  Standalone readiness-report public cryptographic-evidence negatives now also
  require exact route-canary evidence source and `evidence_bound = true` for
  every message-proof launch domain, plus nonzero EVM transaction/receipt/root
  and message identifiers, positive u32 receipt block numbers, and finalized
  receipt flags for ETH/BSC.
  Release-bundle pre-render public cryptographic-evidence validation now
  enforces the same message-proof evidence-source/boundary metadata for ETH,
  BSC, and TRON copied rows and requires complete EVM receipt identifiers before
  Markdown or JSON artifacts can be written.
  Strict published-bundle verification now independently rejects
  message-proof route-canary evidence-source or `evidence_bound` drift across
  ETH, BSC, and TRON public crypto rows, including TRON rows that return
  through the non-active-lane verifier path.
  Solana/TON snapshot public crypto rows now get the same row-local treatment:
  standalone readiness, pre-render bundle validation, and strict
  published-bundle verification all reject snapshot route-canary evidence
  hashes unless the row uses the exact live-snapshot evidence source and
  `evidence_bound = true`.
  All public crypto rows now also reject route-canary source metadata or
  `evidence_bound = true` when the row has no route-canary evidence hash, so a
  copied empty canary cannot imply live evidence in readiness JSON, bundle
  pre-render validation, or strict verification.
  They also reject copied `route_canary_message_proof_used` booleans when the
  route-canary evidence hash is absent, so message-proof usage cannot imply a
  canary that is not present.
  They also reject copied TRON owner/signature flags when the route-canary
  evidence hash is absent, so those transaction-owner predicates cannot imply a
  canary that is not present.
  TRON public crypto rows now also expose and bind the route-canary transaction
  id, transaction owner address, signature SHA-256, and recovered signature
  address. Readiness JSON, release-bundle pre-render validation, and strict
  bundle verification require non-zero canonical transaction/signature hashes,
  non-zero canonical `0x41` owner/recovered addresses, recovered-owner equality,
  and empty TRON-only transcript cells for non-TRON or absent route-canary rows.
  They also reject copied scalar proof context, transcript hashes, and TRON
  block metadata when the route-canary evidence hash is absent, so proof-context
  fields cannot imply a canary that is not present.
  They also reject copied EVM receipt/transaction metadata when no
  route-canary evidence hash is present, so transaction/receipt fields cannot
  imply a canary that is absent.
  Non-EVM route-canary rows with Solana, TON, or TRON evidence now also reject
  copied EVM receipt/transaction metadata, so a live snapshot or TRON message
  proof cannot be relabelled with EVM receipt context.
  EVM and snapshot route-canary rows also reject copied TRON block
  number/timestamp metadata, so TRON DPoS block context cannot be relabelled
  onto non-TRON evidence.
  The raw all-lanes validator now rejects the same cross-lane source-adapter
  gate/audit and route-canary transcript replay before a forged route-canary
  evidence hash can pass through route summary construction.
  Embedded source-adapter gate audit hashes must also stay distinct from
  same-lane route-canary transcript hashes in raw all-lanes validation, direct
  all-lanes release-checklist validation, release-bundle pre-render validation,
  and strict published-bundle verification. Standalone readiness-report public cryptographic-evidence
  negatives now exercise source-gate audit replay of source-role hashes and
  route-canary transcript hashes across every launch-domain row, including
  route-record fallbacks when a forged source-gate hash causes route-allowlist
  recomputation to fail.
  The all-lanes route-canary scalar inventory now pins the exact expected
  evidence source for every active launch lane (`eth`, `bsc`, and `tron`) and
  has readiness/strict-bundle negative tests that remove a
  lane's evidence-source sentinel, so route-canary helper or evidence-source
  drift cannot silently drop one supported lane while the rest of the scalar
  gate remains present.
  The all-lanes release-checklist exact-boolean inventory now has the same
  active-lane coverage guard for `eth`, `bsc`, and `tron`, so
  SDK or core route-canary role-separation sentinels cannot silently lose an
  entire launch lane while the rest of the checklist marker set still passes.
  That exact-boolean inventory now also pins the source-material
  `placeholder_material = False` guard and alias regression, so string,
  container, null, integer, or hostile scalar aliases cannot turn diagnostic
  placeholder material into production all-lanes evidence.
  It also pins the hostile scalar redaction assertions, keeping
  `placeholder_material` alias failures from leaking hook text in public
  blockers.
  The bundle builder's pre-render regression now exercises cross-lane replay
  against another lane's canary evidence hash, source-material hash,
  source-adapter gate audit hash, destination-binding hash, and route-allowlist
  hash, and the strict verifier now rejects the same published-artifact drift.
  EVM-family copied route canaries must also
  preserve non-zero and distinct transcript hash roles, positive/u32 receipt
  metadata, lane-bound target domain, `proof_version = 1`, SORA proof source,
  and finalized message-proof booleans before bundle output is written.
  TRON copied route canaries must likewise preserve canonical signer addresses,
  recovered-signer ownership, non-zero and distinct transcript/governed hash
  roles, positive u64 block numbers, non-negative u64 block timestamps, TRON
  target domain, `proof_version = 1`, SORA proof source, and true owner/proof
  verification booleans before public output is written. Solana copied route
  canaries must preserve a non-zero canonical
  ProgramData address and canonical positive ProgramData slot, while TON copied
  route canaries must preserve non-zero live-account hashes, canonical positive
  transaction LT, and governed hash-role separation before public output is
  written.


<a id="record-8f523644b0bfb414afd5917348497367f8dee7a382c11e75ebe4e65284721f73"></a>

- SCCP release-bundle manifest/readiness roots and public artifact rows must
  classify unknown top-level and artifact field names before artifact closure,
  manifest order, or Markdown table checks. Safe ASCII operator names may remain
  readable, while padded, control-character, whitespace, Markdown-unsafe,
  malformed, or Unicode-confusable keys must be category-only blockers. The
  bundle builder must classify copied report-artifact row field names and reject
  copied report-artifact rows with unknown fields, malformed bundle-relative
  path text, zero, negative, or non-integer byte counts, or noncanonical
  or all-zero SHA-256 text before `--allow-not-ready` diagnostics can render
  or write public artifacts.
  Release-readiness and strict bundle source inventory must pin those direct
  artifact-row regressions so deleting the unknown-field, byte-count, or
  digest-canonicality/non-zero guards blocks public readiness. Public verifier
  summaries and release-note artifact tables must apply the same non-zero
  canonical SHA-256 rule so an all-zero sentinel cannot survive by passing
  through a later render or verification surface. The input-provenance schema
  inventory now also pins strict-bundle type-drift, repeated-numbered metadata,
  zero-byte, digest-text, and all-zero SHA-256 artifact regressions.
  The manifest artifact-set/order inventory now also pins repeated duplicate
  manifest paths, unmanifested entries, unsupported filesystem entries,
  redacted entry-path blockers, manifest artifact metadata errors, and symlinked
  entry diagnostics with representative redacted path names.


<a id="record-45d30a4981b041bc32119d620339811ab47ef3d7122dff9aa0a36dd4e94d1398"></a>

- SCCP cryptographic-evidence public rows must classify unknown row field names
  before lane binding, route-canary binding, Markdown checks, or source-adapter
  audit semantics. Safe ASCII operator names may remain readable, while
  padded, control-character, whitespace, Markdown-unsafe, malformed, or
  Unicode-confusable row names must never be echoed. The release-readiness CLI
  public JSON sanitizer must suppress the copied `cryptographic_evidence` root
  on unknown fields, malformed non-string keys, lane/domain drift,
  noncanonical hashes, malformed booleans, malformed receipt metadata, missing
  required fields, or malformed source-adapter audit hash maps. The bundle
  builder must also classify copied cryptographic-evidence row field names and
  reject copied cryptographic-evidence rows with malformed domain/chain scalars,
  boolean/null fields, optional bytes32 text, optional block-number fields,
  source-adapter audit-hash maps, or audit-hash keys before
  `--allow-not-ready` diagnostics can render or write public artifacts.
  Source-adapter audit hashes in copied cryptographic-evidence rows must also
  stay role-separated from source material, source deployment, destination,
  route-allowlist, route-canary evidence, and route-canary transcript hashes
  before public output is
  written; the standalone readiness JSON sanitizer now enforces the same
  source-gate audit hash-role separation before it can publish copied rows.
	  Public cryptographic-evidence route-canary rows must also keep
	  `route_canary_evidence_hash` distinct from the copied EVM transaction,
	  receipt-block, receipts-root, message-id, TRON transaction-id, and TRON
	  signature-SHA-256 transcript hashes before readiness JSON, bundle
	  Markdown, or strict bundle verification can pass.
	  Message-proof public route-canary rows for ETH/BSC/TRON must also expose
	  exact scalar proof context: a non-negative u32
	  `route_canary_log_index`, `route_canary_target_domain` equal to the lane
	  domain, `route_canary_proof_version = 1`, and
	  `route_canary_proof_source_domain = SORA`; Solana/TON public rows must keep
	  those scalar proof-context fields `null`. Standalone readiness-report
	  negatives now exercise both the ETH/BSC/TRON exactness branch and the
	  Solana/TON null-policy branch across the full launch scope.
	  Message-proof public route-canary rows for ETH/BSC/TRON must also expose
	  exact transcript commitments as non-zero bytes32 fields:
	  `route_canary_call_data_sha256`, `route_canary_payload_hash`,
	  `route_canary_statement_hash`, `route_canary_commitment_root`,
	  `route_canary_finality_height`, and
	  `route_canary_finality_block_hash`; Solana/TON public rows must keep those
	  transcript commitment fields `null`. Standalone readiness-report negatives
	  now exercise both the ETH/BSC/TRON nonzero transcript branch and the
	  Solana/TON null-policy branch across the full launch scope. Copied rows
	  must bind them back to
	  the embedded all-lanes route-canary evidence before readiness JSON, bundle
	  Markdown, or strict bundle verification can pass.
	  Strict public cryptographic-evidence row validation must also require the
	  `enforce_evm_live_tags` control to be an exact boolean before Ethereum/BSC
	  live-tag policy is selected.
	  Release-readiness and strict bundle source inventory must pin the direct
	  public-row schema regressions as well, including zero governed hashes,
  route-canary/source-gate domain-policy drift, exact JSON type drift, and BSC
  testnet row-shape checks.

