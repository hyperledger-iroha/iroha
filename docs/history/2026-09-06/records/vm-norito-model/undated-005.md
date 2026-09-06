# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-454f0b64e1e8e73a5d1a759194d0261dd4fab2705a5c24233e2a06afb3e8ebb9"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- Keep hardening the ISO 20022 bridge after the new inbound lifecycle endpoints
  and durable outbox helpers for `pacs.002`, `pacs.004`, `camt.029`, `camt.056`,
  `sese.023`, `sese.024`, `sese.025`, and `colr.012`; remaining TradFi work is
  led by a consensus-owned permanent economic idempotency claim keyed by the
  rail/profile, ISO message id, payload digest, business message id, UETR, and
  exact signed transaction hash, committed atomically with the transfer. The
  local admission lock, durable exact-hash reservation, refusal to dispatch a
  payment without durable transaction-identity storage, and pinning of pending
  or queued-unsettled records remain tactical protections; bounded node-local
  TTL/count state must not be treated as a network-wide replay boundary.
  Further work is tracked in the engineering backlog for deeper XMLDSig/XAdES
  path-policy processing
  beyond the implemented trust-anchor, signer-admission, key-identifier, and
  revocation corridor; official XMLDSig/XAdES trust-anchor packages; CRL/OCSP
  or rail revocation-feed fixtures; complete canonical XML coverage; and
  broader MDR/XSD validation breadth beyond the checked-in live-profile fixture
  corridor, which now covers `pacs.002`, `pacs.004`, `camt.056`, `sese.023`,
  `sese.024`, `sese.025`, and `colr.012` payment, securities, and collateral
  lifecycle XML, duplicate-free fail-closed profile version and required-BizSvc
  allowlists in runtime and profile-catalog evidence preflight,
  exact runtime binding across BAH/body `MsgDefIdr` values and XML
  namespace-scope-resolved `Document` plus immediate payload-root XSD
  namespaces with the `urn:iso:std:iso:20022:tech:xsd:` prefix, so forged
  concrete MDR-version drift, namespace suffix spoofing, unqualified roots,
  prefixed namespace spoofing, and payload-root namespace rebinding fail before
  profile admission while valid default or prefixed ISO roots still flow to
  profile allowlists when declarations are internally consistent, exact
  real-XML closing-tag matching that rejects mismatched, extra, attributed, and
  unclosed tags before field extraction, fail-closed attribute parsing that
  rejects unquoted, unterminated, malformed trailing, empty-name, unsupported
  name, and duplicate attributes before namespace resolution, XML text and
  attribute value decoding for predefined and numeric character references with
  fail-closed unknown, unterminated, invalid numeric, raw invalid XML-character,
  and raw attribute less-than cases, buffered simple-content text that preserves
  chunks split by comments or processing instructions, mixed-content rejection,
  single-root enforcement, deliberate special-markup handling that accepts
  well-formed comments/XML declarations but rejects malformed comments,
  malformed processing instructions, CDATA, `DOCTYPE`, and other declarations,
  supported-QName validation for elements, attributes, namespace declarations,
  and processing-instruction targets before local-name matching, strict internal
  `<ISO20022>` wrapper parsing for exact root/field tags, exact
  message/path/encoding attributes, supported encodings, canonical field-path
  shapes, and no ignored non-root or nested field markup, JavaScript
  `pacs.009` builder
  namespace derivation from the validated `messageDefinitionId`,
  duplicate-free profile/reference/message-profile config,
  trimmed non-empty profile config literals, duplicate-free trimmed
  trust/revocation material config, CRL/OCSP DER-shape evidence preflight,
  non-overlapping trust-pin alias fields, bounded profile numeric scalars, and
  bounded ISO 4217 minor-unit overrides in runtime and profile-catalog evidence
  preflight, plus official-MDR XSD
  assertions for
  profile-advertised `pacs.004.001.09`/`pacs.004.001.10` and
  `camt.056.001.08`/`camt.056.001.09` return/cancellation variants. An offline
  XSD/XML fixture-manifest preflight now pins checked-in
	  schema target namespaces, `Document` payload roots, fixture namespaces, and
	  reviewed missing-schema exceptions, while requiring schema/fixture identifier
	  material and schema attribute names to remain printable ASCII before mismatch
	  diagnostics can quote them, rejecting copied XML fixtures with duplicate
	  fixture SHA-256 values, non-canonical schema/fixture path segments, and
	  optionally validating schema-backed XML fixtures against their checked-in XSDs
	  with `xmllint --nonet`; it also
  requires canonical repository/commit/path/license/source-SHA provenance for
  every checked-in XSD with source repository URLs and source paths capped at
  2048 characters, lowercase canonical GitHub owner/repository coordinates
  required during preflight and readiness replay, owner coordinates restricted
  to lowercase alphanumerics and non-edge hyphens, repository names required to
  contain at least one lowercase alphanumeric character, placeholder repository
  owners or names including separator-obfuscated or collapsed marker variants,
  and all-zero Git commit or SHA-256 provenance placeholders
  rejected during preflight and readiness replay, secret-looking repository coordinates
  rejected before archived-summary output, identifier-only or key/value
  whitespace/dot/underscore/hyphen separated secret key labels rejected in
  percent-decoded ISO CLI paths plus live/archived response previews, regex-only
  bearer whitespace forms rejected in response previews, invalid UTF-8 or
  non-ASCII successful live response previews rejected, failed live previews
  redacted with accepted newline/tab text folded to one line, archived
  multiline or non-ASCII receipt previews/errors rejected, URL transport
  receipts fixed to rail/notary labels
  instead of upstream reason strings, archived failed receipt errors bounded to
  those labels or matching `HTTP <status>` labels, JSON/XML parser diagnostics
  category-only without parser location payloads, accepted printable XSD
  `xmllint` and direct receipt-verifier stderr folded to one line before
  diagnostics are composed, and non-ASCII or identifier-style secret-looking
  path material rejected before summary emission, archived executed rail/notary
  canary stdout required to parse as live adapter summary JSON with zero
  failures, stage-scoped receipt paths, matching receipt counts, rail
  `submitted_messages` bound to explicit `--message`, and notary
  `endpoint_count`/`published_anchors` plus latest-vs-all anchor paths bound to
  the executed `--endpoint` and `--all` flags, sanitized compact
  `stage_command_modes` added so production-readiness replay can independently
  reject forged rail single-message, rail submitted-message-count, notary
  all-anchor, notary endpoint-count, or notary published-anchor-count receipt
  summaries without retaining raw command lines, canary
  verify-stage receipt-verifier stdout paths required to be covered by the
  captured verify command selectors, adapter stdout receipt path sets required
  to match verifier stdout paths by receipt kind, and direct
  XSD summary output
  paths forbidden from reusing or
  hardlinking manifest/profile-catalog input files, while requiring the
  `blocked_schema_sources` and `pending_schema_sources` review lists to be
  recorded explicitly even when empty, use unique `message_def_id` values, and
  match a current missing-schema fixture gap or, with a profile catalog, a
  current profile-version gap,
  rejects XSD files with known restricted Standards
  Editor redistribution terms, parses the embedded default rail profile catalog
  on demand, and records which concrete advertised message versions are
  schema-backed while rejecting unknown profile/message catalog keys before
  release evidence is emitted; final production-readiness replay now recomputes
  those profile-version `schema_backed` flags from schema-backed XML fixture
  message-definition IDs before accepting archived summaries, and archived XSD
  summaries bind a direct raw-derived unreviewed unique profile-message gap
  count/list that readiness rechecks before emitting production blockers; the
  first-release checked-in manifest and embedded default profile catalog now
  advertise only schema-backed concrete message definitions, so current
  repository summaries report zero missing profile schema versions while
  retaining synthetic reviewed-gap tests for pending/blocked evidence replay;
  direct verification and readiness replay still pin known pending message
  definitions to their exact recorded ISO catalogue URLs,
  direct download URLs, download type, message names, and submitting
  organisations, and also pin first-release blocked public candidates to their
  audited source provenance plus restriction-marker list while rejecting blocked
  rows for definitions already tracked as official pending ISO sources;
  pending direct download URLs must be unique within each summary
  and across archived summary replay, pending official ISO catalogue/download
  URLs must not contain percent escapes, archive catalogue URLs must use
  canonical raw `page=<nonzero decimal>` queries, and pending source message
  names must be unique and use canonical ISO-style CamelCase plus `VNN` suffixes
  that match the corresponding `message_def_id` version segment;
  pending submitting-organisation labels must be bounded canonical ISO-style
  comma-space-separated names without URL/contact delimiters, semicolon path
  parameters, placeholders, or path-like slash smuggling;
  `scripts/iso_pending_xsd_source_probe.py` now emits bounded, digest-stamped
  reachability summaries for those pending official ISO downloads, including
  `sample_sha256` over exactly the capped downloaded byte sample and `null` for
  zero-byte failures, without importing schema bytes, so operators can re-probe
  and archive the remaining official-package blocker deterministically; it
  rejects malformed repeatable
  selectors plus non-ASCII, padded, or non-canonical timeout/byte-cap numerals,
  caps timeout numerals at 300 seconds, and rejects raw secret-looking CLI
  arguments plus malformed `--summary-out` path tokens before network work or
  argparse echo; it also omits unsafe remote `Content-Type` metadata and records
  only real integer 100-599 HTTP statuses before evidence digesting, and
  normalizes network failures to a stable `NetworkError` role; final readiness
  now rejects forged archived `content_type` values that the probe producer
  would have omitted, including whitespace-padded or secret-looking header
  material, and failed response header access omits `content_type` without
  archiving accessor exception text, while runtime read failures normalize to
  `NetworkError` without retaining exception text;
  final readiness now accepts `unexpected` pending-probe status only for a real
  1xx/2xx/3xx HTTP response with positive bounded sample bytes and
  `looks_like_xsd=false`, and reserves `reachable` pending-probe status for
  real 2xx responses with positive bounded bytes and an anchored,
  namespace-bound XML Schema root opening tag whose `targetNamespace` matches
  `urn:iso:std:iso:20022:tech:xsd:<message_def_id>`, not just an XML
  declaration, embedded marker, or generic schema root, while redirect-class
  or wrong-target XSD-looking samples fail closed as non-production evidence,
  while malformed status metadata, malformed status accessor failures, opener
  or response context failures, zero-byte success responses, malformed non-byte
  read output, and stream read failures normalize to `NetworkError` without
  archived samples, and bytes-like
  read outputs are sliced to the
  configured bounded window by byte length before digesting or classification
  even if a response object over-returns or returns a wide-format `memoryview`
  while `truncated` still records cap overflow;
  rail/notary live receipt transport errors now redact local-path-shaped
  material before persistence, and receipt verification rejects forged archived
  receipt `error` strings that reintroduce local path material;
  direct XSD summaries now also emit nested profile-catalog `versions`,
  `missing_schema_versions`, and `skipped_family_versions` in canonical
  profile/message/direction/version order, and final readiness rejects
  digest-correct reordered nested catalog arrays;
  catalog
  `versions` lists can skip schema-backed checks only for the exact
  message-family alias, not arbitrary strings, and
  runtime-required catalog fields are required while optional catalog fields are
  shape-checked when present, including fail-closed trust/revocation pin overlap,
  bounded CRL-like/successful Basic OCSP DER material checks in embedded core
  loading and offline preflight, and malformed configured CRL/OCSP DER
  plus over-limit revocation-material rejection in Torii runtime overrides,
  with embedded core, offline preflight, and runtime overrides sharing the same
  `8`-entry revocation-material list cap; optional manifest/profile
  fields are optional only when omitted, so present `null` reviewed reasons,
  trust/revocation material lists, booleans, numeric caps, business-service
  arrays, or amount minor-unit arrays fail before digest-bound XSD/profile
  evidence is emitted.
  All checked-in
  XSDs now
  have standalone XML fixtures that pass XML schema validation, and the
  checked-in default profile scope passes the strict schema-backed and
  profile-version release flags. Remaining MDR/XSD work is locating
  redistributable official packages before expanding default profiles with
  additional concrete payment, securities, and collateral lifecycle versions. Blocked public
  XSD candidate evidence now must carry at least one explicit redistribution or
  public-distribution restriction marker, and checked-in XSD preflight
  normalizes license-header whitespace and zero-width format characters before
  matching known restricted redistribution phrases, so copyright-only
  provenance or obfuscated restricted terms cannot satisfy missing-package
  blocker evidence. Pending official ISO source evidence is reviewed
  catalogue/download URL and message-name provenance only, with byte-stable
  percent-free bounded official URLs, canonical raw archive page queries, unique
  direct download URLs, and unique canonical message-name version suffixes bound
  to the `message_def_id`, and never substitutes for checked-in XSD bytes. The
  retired `colr.007` collateral parser and route are no longer part of the
  first-release runtime surface; operator receipt/evidence/readiness gates
  reject retired rail receipts and stale legacy summary fields with no override. An
  aggregate ISO production-readiness rollup now requires explicit expected
  provider/environment context, non-empty strict XSD proof, operator evidence
  summaries, and digest-bound direct receipt-archive verification with
  unique canonical per-receipt `*.receipt.json` paths, digests, and successful
  2xx receipt status plus kind-specific notary/rail metadata into one release
  gate; remaining readiness work is making that gate pass without diagnostic
  overrides and with real provider evidence. Default-profile rail canary
  evidence must also carry an explicit `--default-rail-profile` binding so
  `profile=null` receipts prove trust coverage for the configured fallback,
  and custom-profile canary/archive receipts must use a message family covered
  by the matched trust profile's schema-backed first-release rail family
  instead of relying on an implicit Torii default or runtime-only lifecycle
  endpoint, and production readiness replays that binding against compact trust
  profiles before the aggregate can pass. Trust-bundle verifier summaries now canonicalize raw
  bundle, DER-material, pin, and OID list order, direct evidence replay rejects
  digest-correct raw trust-summary reordering, and final readiness blocks
  digest-correct compact trust DER proof reordering. Canary command planning
  now canonicalizes repeatable notary endpoint and verify receipt selectors,
  while evidence replay rejects digest-correct child-command selector
  reordering. Live rail/notary response bodies now normalize `bytes`,
  `bytearray`, and `memoryview` values by byte length before hashing or
  previewing, reject non-byte response bodies as stable transport failures
  without echoing the value, and detect wide-format `memoryview` cap overflow
  by byte count. Final readiness also canonicalizes top-level XSD, evidence, and
  pending-probe summary references plus blocker/reviewed-gap warning and nested
  diagnostic-entry order before digesting release summaries, and canonicalizes
  loaded summary lists before cross-summary blockers are generated so
  duplicate/replay diagnostics are input-order stable. Receipt-summary entry
  order is also checked independently of receipt-digest validity, so malformed
  receipt digests cannot hide noncanonical canary or archive receipt replay
  order. Unsupported receipt-kind and unsupported, malformed, or secret-looking
rail `message_type`, `profile`, `rail_message_id`, and `source_path` values
are scrubbed to `"unsupported"` in readiness output while blockers remain
label-only; malformed or unsupported notary `anchor_sha256`, `index_sha256`,
`anchor_path`, `store_dir`, `index_path`, and `record_count` values are now
scrubbed the same way. Unsupported summary `version` values in XSD,
pending-probe, canary,
receipt, and trust summaries are similarly normalized to `"unsupported"` in
final readiness output, and mismatched archived evidence policy, canary, and
trust profile provider/environment context values are also normalized to
`"unsupported"`. Release and archived freshness budgets are capped at 36,500
days, and weaker archived evidence/trust source budgets are normalized to
`"unsupported"` in final readiness output. Blocked compact trust verifier
override flags, receipt verifier `allow_*` policy flags, and
`require_source_files=false` are also normalized to `"unsupported"` in final
readiness output. It accepts
digest-bound pending-XSD source probe summaries for reviewed
  pending-source gaps, and rejects missing,
  stale, metadata-mismatched, bad-sample-digest, over-timeout-summary,
  over-cap-summary, oversized-sample, impossible failed-probe bytes, impossible
  truncation, timeout/network-error content-type spoofing, HTTP-error
  success-code replay, wrong failed-probe error-kind roles, or unreachable
  probe evidence without treating probes as schema-backed closure. It blocks supplied probe
  summaries when no
  pending official source gap references them, also blocks extra probe rows
  when only a subset of official source gaps remains, and rejects
  probe-summary paths and summary digests replayed as XSD or operator-evidence
  material. It rejects
  XSD summary, manifest, schema, fixture, blocked-source, and profile-catalog
  artifact paths replayed under a different XSD material role, so a copied
  summary, manifest, catalog,
  schema, fixture, or blocked-source path cannot satisfy another proof class,
  and rejects those XSD artifact paths when replayed as evidence summary,
  canary, trust, receipt, config, or bundle path material.
  ISO operator and release CLI numeric preflights now also enforce canonical
  printable-ASCII decimal spellings for timeout, freshness-budget, and byte-cap
  flags before argparse conversion, preserving the existing typed positive
  finite/integer validators while rejecting padded, plus-signed, signed-zero,
  and overflow-exponent aliases. The pending XSD source probe now preflights
  timeout and byte-limit values before argparse can treat negative numeric
  spellings as options.
  Evidence and final-readiness provider/environment context preflights now also
  reject leading-dash separate and equals-form values, and evidence default
  profile ids reject leading-dash values, before argparse can reinterpret those
  identifiers as options.
  Live rail gateway and audit notary URL preflights now reject leading-dash
  `--torii-base-url` / `--endpoint` values at the same raw-CLI boundary.
  Archived canary child-command replay now rejects leading-dash separate and
  equals-form rail/notary endpoint URL and bearer-token-file values before they
  can be treated as valid stage arguments.
  Durable ISO state now has
  versioned per-record digests plus a local
  tamper-evident audit index exposed through the
  `GET /v1/iso20022/audit/messages` route, with config-backed age/count
  retention/compaction, an `audit_export_dir` manifest/notary-preimage spool,
	  and an operator adapter that verifies canonical nonzero anchor and audit-index
	  self-digests before publishing those preimages to clean
		  raw-whitespace-free, canonical-host/label/port/path,
		  printable-ASCII-path, raw-delimiter/percent-smuggling-free,
		  overlong-URL/host-rejecting,
		  localhost/private-IP/rebinding-host/legacy-IPv4/IPv6-transition-rejecting,
		  duplicate-free HTTPS archival/notary endpoints with
				  regular non-symlink bounded exact runtime bearer-token files, rail drop
						  roots and inputs that reject symlink leaves and symlinked ancestors,
						  including explicit message leaves with
				  whitespace/leading-dash-segment/backslash/semicolon/empty-segment/dot-segment smuggling rejected
				  before reads and all-zero rail sidecar payload digests, duplicate payload
				  digests, or duplicate rail message ids rejected before network delivery, with live rail and notary redirects
			  archived as failed receipts instead of followed, and with live rail, notary, and archived
			  receipt endpoint URLs capped for full URL length and DNS-host length
			  and rejecting reserved placeholder hosts such as `.example` or
			  `example.invalid` plus raw, encoded, or non-ASCII path delimiter smuggling,
	  bearer-token files for the live rail and notary adapters capped before
	  decoding,
			  regular non-symlink notary export roots/source files with symlinked
			  ancestors rejected, 64 MiB caps for anchor/index JSON artifacts,
			  1 MiB caps for persisted record-source JSON artifacts, and local
			  receipts that do
				  not persist token material, reject secret-looking or control-bearing
				  successful remote response bodies before receipt persistence, redact
				  failed remote response previews and secret-looking, control-bearing,
					  oversized, non-ASCII, or unstringifiable transport errors, normalize
					  non-standard, malformed, or oversized remote HTTP statuses to
					  `status_code=null` with label-only invalid-status errors, reject
					  boolean or string status aliases before coercion, convert transport-open
					  exceptions/failures, normal/HTTP-error close failures, response-read exceptions/failures, and malformed non-byte
					  remote response bodies into bounded failed receipts, and preflight receipt output
			  directories/leaves before input loading, publication, or Torii submission, rejecting
		  control characters, whitespace, leading-dash segments, backslashes,
		  semicolon parameters, URI/drive prefixes, malformed or smuggled percent escapes, empty segments, dot/parent traversal, symlinked existing
		  ancestors, hard-linked outputs, or symlinked outputs, and using
		  owner-private descriptor-checked same-directory temporary files
			  with bounded digest-derived names plus atomic replacement where
			  available, and live rail/notary adapter timeout and byte-cap CLI
				  values now fail closed on non-positive or non-finite inputs before
				  local reads or network delivery, every ISO operator CLI rejects
				  control-bearing unknown raw arguments before argparse diagnostics,
				  requires unknown raw arguments to remain printable ASCII,
				  preflights required notary/rail URL values for control,
				  non-ASCII, whitespace-padded, and non-URL-shaped secret-looking
				  material before unrelated local path checks,
					  keeps URL host labels printable ASCII before host/IP
					  numeric-label checks can accept Unicode digit confusables,
					  rejects archived canary command flags with secret-looking or
					  non-ASCII spellings before echoing unsupported flag names,
					  reports non-ASCII, overlong, too numerous, or collectively oversized unknown JSON keys with label-only diagnostics,
					  rejects non-ASCII receipt-kind spellings before unsupported-kind
					  diagnostics can echo them,
					  rejects non-ASCII trust embedded-signature policies before
					  unsupported-policy blockers can preserve them,
					  keeps trust source authority/version provenance printable ASCII
					  before direct or archived summaries can preserve it,
					  scrubs non-production/unsupported trust policies and placeholder
					  trust-source authority/version/URL values from final readiness output,
					  normalizes unsupported summary `version` values in XSD,
					  pending-probe, canary, receipt, and trust summaries to
					  `"unsupported"` before final readiness output,
					  normalizes mismatched archived evidence policy, canary, and
					  trust profile provider/environment context values to
					  `"unsupported"` before final readiness output,
					  caps release and archived freshness budgets at 36,500 days
					  and normalizes weaker archived evidence/trust source
					  budgets to `"unsupported"` before final readiness output,
					  normalizes blocked compact trust verifier override flags,
					  receipt verifier `allow_*` policy flags, and
					  `require_source_files=false` to `"unsupported"` before final
					  readiness output,
					  normalizes blocked canary `plan_only=true`,
					  `require_explicit_policy=false`, per-stage
					  `stage_dry_run=true`, and compact trust
					  `profile_json_emitted=false` / `profile_json_emittable=false`
					  states to `"unsupported"` before final readiness output,
					  normalizes missing compact trust source objects
					  (`source=null`) to `"unsupported"` before final readiness
					  output,
					  normalizes compact trust `max_source_age_days=null` to
					  `"unsupported"` when profile JSON is not emittable before
					  final readiness output,
					  normalizes failed pending-XSD probe `ok=false` and
					  `looks_like_xsd=false` states to `"unsupported"` before
					  final readiness output,
					  normalizes non-success pending-XSD probe response/sample
					  fields (`http_status`, `content_type`, `downloaded_bytes`,
					  `sample_sha256`, `truncated`, and `error_kind`) to
					  `"unsupported"` before final readiness output,
					  normalizes missing XSD strictness proof flags to
					  `"unsupported"` before final readiness output,
					  normalizes failed or malformed receipt entry `ok` values to
					  `"unsupported"` before final readiness output,
					  preserves receipt response metadata triplets only for
					  internally coherent successes (`ok=true`, 2xx integer
					  `status_code`, and canonical nonzero
					  `response_body_sha256`) and normalizes failed, mismatched,
					  incomplete, or malformed triplets to `"unsupported"` before
					  final readiness output,
					  normalizes retired rail message types and
					  default-profile fallback `profile=null` receipt values, plus
					  `endpoint_requires_insecure_http=true` markers, to
					  `"unsupported"` before final readiness output,
					  normalizes diagnostic trust-source URLs that require
					  `allow_insecure_source_url=true` to `"unsupported"` before
					  final readiness output,
					  keeps direct and archived trust DER labels printable ASCII
					  before summaries can preserve them,
					  keeps provider/environment context labels printable ASCII from
					  canary/trust generation through evidence verification and release
					  readiness and reports context mismatches without printing observed
					  or expected values, rejects archived canary stage names with non-ASCII
					  confusables before unsupported-stage diagnostics can echo them,
						  keeps canary runbook paths and archived child-command paths
						  printable ASCII and within the 4096-character local path cap,
						  while readiness compact summary/config/receipt paths stay
						  within the stricter 2048-character archive cap
						  before release evidence can preserve path confusables,
					  the unsupported `--` argument terminator before trailing values can
				  bypass raw secret, boolean, path, context, or numeric preflights
				  or be echoed by argparse, disable argparse long-option abbreviation
				  so partial flag spellings cannot bypass exact preflight matching,
				  and live
				  adapter local diagnostic
				  flags now reject unused `--allow-insecure-http`,
				  `--allow-default-profile`, and
				  notary `--allow-missing-record-sources` before dry-run summaries
				  network delivery, or receipt output; a
	  read-only receipt verifier now gates those receipts for canary use and emits
	  a digest-bound summary with per-receipt `receipt_sha256` entries while
	  rejecting all-zero raw receipt self-digest placeholders and
	  non-boolean direct policy flags before selector discovery,
	  rejecting unused local verifier overrides for failed receipts, insecure/local
	  endpoints, and missing rail profiles,
		  closing raw receipt and notary source schemas including duplicate-free nested audit records,
		  complete audit-index record key sets,
			  complete persisted record/context/metadata/history key sets for source replay and
			  4096-character clean metadata string caps across notary/receipt audit
			  indexes, persisted records, nullable context/metadata/history fields, and
			  rail sidecars,
			  4096-character direct trust-bundle generic string/OID-list, XSD
			  profile-catalog generic string/list, canary runbook generic
			  string/list, evidence replay clean string/list, and readiness
			  compact clean string/list caps before trust preflight, XSD profile
			  validation, planning, archive replay, or final readiness replay, with
			  embedded trust/profile DER base64 retaining its decoded-size guard,
			  final-readiness `xsd.repository_fixture_manifest` blockers for
			  summaries still generated from the checked-in ISO fixture manifest
			  even in local diagnostic mode, plus
			  `xsd.repository_xsd_summary` blockers for archived summary paths
			  under the checked-in ISO fixture corpus, and
			  `xsd.repository_profile_catalog` blockers for archived
			  profile-catalog paths that point back at those fixtures,
			  evidence/readiness blockers for canary `config_path` values that still
			  point at checked-in `fixtures/iso20022/operator_canary/` runbook
			  templates, plus live canary preflight failures for non-plan
			  config/stage/explicit verifier receipt paths under
			  `fixtures/iso20022/` and evidence replay failures for executed or
			  planned child-command path flags that reintroduce those fixtures, plus
			  direct receipt-verifier and evidence-gate selector failures for
			  `--receipt` and `--receipt-dir` paths under `fixtures/iso20022/`
			  before discovery or child verifier launch,
			  evidence/readiness blockers for compact XSD/evidence/canary/trust
			  summary paths under repository ISO fixture coordinates,
			  trust-bundle source-path retention plus evidence/readiness blockers
			  for compact trust profiles that still point at checked-in
			  `fixtures/iso20022/trust_bundles/` templates,
			  rail receipt `source_path` retention plus receipt/evidence/readiness
			  blockers and adapter preflight failures for checked-in
			  `fixtures/iso20022/*.xml` payload fixtures,
			  notary receipt `anchor_path`/`store_dir`/`index_path` retention
			  plus evidence/readiness replay that binds `latest.notary.json` or
			  digest-addressed `anchors/<index_sha256>.notary.json` paths,
			  `messages.index.json` peers, and source stores into direct archive
			  metadata matching and rejects checked-in `fixtures/iso20022/`
			  anchor/store/index artifacts, with adapter preflight failures for
			  checked-in notary anchor/store fixture inputs,
			  Torii durable-store reload,
	  audit record filename/message-id bindings, Torii reload clean-string enforcement,
	  Torii reload filename/message-id binding, symlink-free regular-file-only Torii record
		  directory/loading, symlink-free Torii durable-output directories, bounded Torii
		  persisted-record persist/reload, endpoint-digest bindings,
		  timestamp/status consistency, required HTTP response digest/error metadata,
		  all-zero response-body, notary anchor/index, rail payload,
		  audit-index self-digest and record, and persisted payload-hash placeholder rejection, bounded response
				  metadata with redacted-marker rejection for successful receipts, canonical receipt endpoint,
				  timestamp, canonical notary/rail source paths, including `store_dir`,
				  that are not flag-shaped, `.xml` rail payload leaves,
			  and whitespace-free rail
				  metadata identifiers, with live rail sidecars rejecting non-ASCII
				  or malformed `message_type` values before unsupported-message echo,
				  explicit `null` `profile`/`rail_message_id`
				  values, non-canonical profile IDs, and overlong or non-canonical
				  ASCII rail-message identifiers before submission, with oversized or
				  unknown-field sidecars rejected as malformed,
	  ASCII-only rail `message_type` digit validation across direct receipt
	  verification, evidence replay, readiness replay, and XSD profile catalogs,
	  ASCII-only XSD profile-catalog `message_def_id` and version validation,
	  overlong XSD profile-catalog profile IDs, enum values, and
	  business-service entries rejected before duplicate-ID, missing-schema-version,
	  unknown-value, or summary echo,
	  overlong XSD/XML schema and fixture identifiers rejected before mismatch echo,
	  label-only XSD source filename, schema namespace/payload-root, fixture
	  namespace/payload-root, unknown schema-reference, and linked schema/fixture
	  mismatch diagnostics,
	  overlong trust-bundle/evidence/readiness compact trust profile IDs,
	  override IDs, policies, and trust-source authority/version/timestamp
	  provenance rejected before trust replay, summary archive, or blocker echo,
	  plus generic evidence/readiness archive/canary kind, filename, or metadata
	  mismatch blockers that do not print receipt kind values, receipt leaf names, or invalid metadata tuples,
	  rail receipt metadata recording for nullable raw receipt fields and retained
	  archived receipt-summary identifiers, rail sidecar source bindings that reject
	  explicit-null optional metadata instead of treating it as omission,
	  notary anchor-path shape checks even when
	  source files are not required, notary anchor/index source bindings and
	  canonical nonzero notary anchor/index self-digests that
	  require regular non-symlink files, notary adapter publication that requires
	  `store_dir/messages` for non-empty anchors by default, persisted notary
			  record-source bindings from each explicit index `records[]` row's
			  `record_sha256` to clean `store_dir/messages` paths, production evidence rejection of the adapter's
		  local `--allow-missing-record-sources` diagnostic override,
		  status-history timestamp binding for persisted record sources, and
		  symlink-free receipt archive directories,
	  and a strict JSON-runbook canary runner rejects
  whitespace-padded or control-bearing runbook strings, non-ASCII runbook path strings, present-null optional
	  path/numeric limit fields, non-canonical JSON number spellings for stage budgets,
	  XSD fixture verification rejects non-canonical JSON number spellings in manifests
	  and profile catalogs, rail gateway sidecars and audit notary anchors/indexes/record
	  sources reject non-canonical JSON number spellings before network delivery,
	  direct receipt verification rejects non-canonical JSON number spellings in receipt files,
	  trust-bundle verification rejects non-canonical JSON number spellings in bundle files,
	  operator evidence rejects non-canonical JSON number spellings in summary/stdout
	  replay, final readiness rejects non-canonical JSON number spellings in release-summary inputs,
	  and every JSON float parser rejects negative-zero floats plus overflow exponents
	  that would normalize to non-finite floats,
	  embedded-whitespace/leading-dash-segment/backslash/semicolon/dot-segment
	  path smuggling including raw URI/drive prefixes, encoded control/space
	  bytes, encoded dot/separator bytes, encoded semicolon parameters, encoded
	  URL delimiters, encoded percent bytes, malformed bracketed hosts, overlong endpoint URLs or DNS
	  hosts, localhost/private-IP/rebinding/legacy-IPv4/IPv6-transition endpoint URLs, and duplicate endpoint and receipt inputs before
	  executing the rail/notary/verify path with one
	  bounded summary, bounding each child stage with positive finite
	  `--stage-timeout-secs`, recording `timed_out` for killed children, draining
	  child stdout/stderr through a configured preview cap, treating preview
	  truncation, unsafe control-character previews, and successful child stderr
	  as failed canaries, treating disabled non-plan verify stages as failed
	  canaries, requiring the verify stage to select generated non-dry-run
	  rail/notary receipt directories, requiring explicit
	  notary and verify receipt-selector arrays under
	  `--require-explicit-policy`, requiring rail/notary receipt directories to
	  be explicit and non-overlapping with the rail inbox or notary audit-export
	  root under that same production-policy flag, rejecting rail/notary receipt
	  directories that overlap their configured bearer-token file paths, rejecting
	  rail/notary bearer-token file paths that overlap stage source roots before
	  child execution, rejecting generated receipt verification when
	  `verify.require_source_files=false`, and capping runbook
	  JSON at 64 KiB before parsing.
	  The operator scripts reject duplicate JSON object keys, non-standard
	  `NaN`/`Infinity` JSON constants, and lone UTF-16 surrogate escapes across
	  runbooks, sidecars, anchors/indexes, receipts, trust bundles, XSD
	  manifests/profile catalogs, evidence summaries, readiness summaries,
	  embedded receipt-verifier stdout, and direct archive receipt-verifier stdout
	  before semantic validation, so shadowed keys, non-finite numbers, and invalid
	  Unicode strings cannot rewrite release evidence. Canonical digest encoders
	  and summary/receipt/profile JSON writers also use `allow_nan=false`, so
	  internal non-finite numbers cannot be emitted as digest-stamped evidence.
	  Direct numeric CLI
	  preflights also reject Unicode digit confusables before Python parsers can
	  accept them as timeouts, byte limits, or evidence age budgets. Those gates
	  also reject symlinked or
  non-regular canary runbooks, trust bundles, evidence/readiness summaries, XSD
	  manifests, profile catalogs, schema files, and XML fixtures before digest,
	  provenance, or policy checks run, opening those inputs through no-follow file
	  descriptors where available. Summary/profile/receipt output paths now also
	  reject checked-in `fixtures/iso20022/` artifact destinations during
	  run-level preflight and again before parent creation or temporary output
	  writes. Production-readiness direct `run(args)` calls now also preflight
	  XSD summary, evidence summary, and summary-output path smuggling before
	  input loading, caps repeatable XSD/evidence summary path lists at 64 entries
	  before loading any summary files, and final summary outputs cannot exactly reuse or hardlink
	  XSD/evidence summary inputs before they are loaded, while keeping checked-in
	  fixture summary inputs as structured release blockers; compact trust summaries with
	  `allow_insecure_source_url=true` can replay `http://` or local/private
	  trust-source URLs into blocker output instead of aborting as malformed. Direct
	  receipt-verifier `run(args)` calls now mirror CLI receipt selector
	  path-smuggling preflights before discovery or file loading.
	  Direct XSD/trust verifier `run(args)` calls also
	  preflight manifest/profile-catalog, bundle, profile-output, and
	  summary-output path smuggling before manifest or bundle loading; direct
		  trust-bundle summary/profile outputs also cannot exactly reuse or hardlink
		  bundle inputs, and summary/profile output paths cannot alias each other
			  before bundle loading; their existing ancestors and leaves are also
			  preflighted without creating missing parents before bundle validation. Direct XSD summary outputs likewise cannot reuse or
		  hardlink manifest/profile-catalog inputs or discovered schema/XML fixture
			  inputs before writing, and preflight summary-output existing ancestors and
			  leaves without creating missing parents before manifest loading or optional `xmllint` subprocess execution;
	  direct operator evidence verifier `run(args)` calls reject
	  `--summary-out` paths that exactly reuse or hardlink canary/trust summary
	  or direct receipt inputs before those summaries are loaded, and reject
	  summary outputs under supplied receipt archive directories, so aggregate evidence output
	  cannot overwrite its source summaries. Evidence summary outputs now also
	  reject symlinked existing ancestors plus symlink, hardlinked, or
	  non-regular leaves without creating missing parents before summary loading
	  or direct receipt-verifier subprocess execution. Final readiness summary
	  outputs reject the same target shapes without creating missing parents
	  before XSD or evidence summary loading. Ancestor inspection failures during
	  these preflights now use role labels and sanitized OS details, or generic
	  `I/O error` for runtime/type/value failures, instead of surfacing
		  attacker-controlled path or secret text from `lstat()`. Reader, input
		  directory, receipt-directory, receipt source-file, output parent/leaf, and
		  alias `lstat()`/`stat()`/`exists()`/`is_symlink()` inspection failures use
		  the same role-label diagnostics before receipt, summary, or emitted
		  profile writers create or replace artifacts; output parent creation and
		  rail/notary receipt-directory inspection or creation failures now use
		  sanitized labels as well.
	  Direct canary/rail/notary adapter
	  `run(args)` calls mirror their CLI
	  path-smuggling guards before config, inbox/export, receipt, token, or
	  network loading. Direct canary summary-output existing ancestors and leaves
	  are preflighted without creating missing parents before runbook JSON
	  loading, with parsed stage-artifact
	  alias checks still running before child execution. Live rail/notary adapter runs also reject inbox/export
	  roots under checked-in `fixtures/iso20022/` artifacts before discovery,
	  fixture parsing, or network delivery, and reject receipt output directories
	  that exactly reuse or symlink back to the rail inbox or notary export root
		  before source discovery. Direct rail/notary adapter receipt directories also
		  reject overlap with explicit rail XML/sidecar source paths, rail/notary
		  bearer-token file paths or directories containing those token files, and
		  notary `latest.notary.json`, `anchors/`, or `messages.index.json` source
		  material before source loading or delivery. Explicit receipt-directory
		  symlink ancestors are rejected before source loading without creating
		  missing output directories, and bearer-token file paths
		  cannot overlap the rail inbox or notary export root before token loading.
	  Direct CLI artifact paths for live rail inbox
  roots, live notary export roots, rail/notary bearer-token files, canary
  configs, trust bundles, XSD manifests/profile catalogs, receipt
	  files/directories, canary/trust summaries, and XSD/evidence summaries reject
	  control characters, whitespace, leading-dash segments, backslashes, semicolon
	  parameters, empty segments, and dot/parent traversal before argparse `Path`
			  normalization or file discovery, and direct local CLI/output/artifact
			  path strings are capped at 4096 characters before secret scanning,
			  filesystem expansion, summary emission, child command construction, or
			  archive replay. Live rail/notary adapter timeouts also
		  reject non-positive or non-finite CLI values, response-body retention
		  caps above 4 MiB, and non-positive byte caps before local reads or
		  network delivery. The receipt
		  verifier caps raw receipt JSON at 4 MiB, notary anchor/index JSON at
		  64 MiB, persisted notary record-source JSON at 1 MiB, rail source XML
		  at 4 MiB, and rail source-sidecar JSON at 16 KiB before source replay,
			  while the notary adapter and downstream evidence gates require positive
			  notary record counts, canonical audit-index lifecycle states, and
			  state-compatible pacs.002 summary/status-history codes before publication
			  and during source-file or production-evidence replay. The
		  evidence gate caps
	  direct receipt-verifier stdout/stderr at 4 MiB before JSON parsing, bounds
	  direct verifier runtime with positive finite `--receipt-verifier-timeout-secs`,
	  redacts secret-looking, control-bearing, non-ASCII, or local-path-shaped
	  verifier stderr diagnostics, and
	  rejects control characters, whitespace, leading-dash segments, backslashes,
  semicolon parameters, empty segments, dot/parent traversal, and symlinked
  receipt, summary, and emitted profile-override outputs before writing them.
  Canary summary outputs and runbook artifact paths
  are preflighted before subprocess stages, summary outputs cannot exactly
  reuse or hardlink the runbook config input before config loading, cannot
  reuse planned rail/notary bearer-token files, explicit rail message files, or
  explicit verifier receipt files, and cannot be written under planned stage
  artifact directories before child execution, and canary
  relative paths preserve final leaves after parent containment checks so child
  scripts can still reject symlinked leaves. Archived canary/trust summaries consumed by the evidence
	  gate and XSD/evidence summaries consumed by the readiness gate are capped at
	  4 MiB before parsing, optional `xmllint` stdout/stderr is capped at 64 KiB
	  and runtime is bounded by positive finite `--xmllint-timeout-secs` capped
	  at 300 seconds during
  XSD fixture validation, with successful validator output limited to empty
	  output or the normal `<fixture> validates` line and secret-looking,
	  control-bearing, or non-ASCII validator diagnostics redacted, operator trust-bundle JSON is capped at
  64 MiB before trust-preflight parsing, and trust DER base64 is capped before
  decoding to the 1 MiB DER material limit while requiring every trust-material
  SHA-256 pin and each DER object digest to be canonical and nonzero, with DER
  object digests also matching the decoded DER bytes. Trust-bundle preflight now requires an
  explicit `embedded_signature_policy` instead of inferring `require-verified`
  from omission, and every list-typed trust-material field must be recorded as
  an array so intentionally empty pin/DER collections cannot be confused with
  omitted production evidence.
  An offline
	  evidence gate now requires exact expected provider/environment context,
	  records that context in its digest-bound policy, rejects all-zero archived
	  summary digest placeholders before mismatch diagnostics, recomputes
  canary/trust/receipt summary digests, rejects repeated or copied
  canary/trust summaries, rejects non-canonical or duplicate receipt paths or
  receipt digests, rejects duplicate archived trust profile IDs and bundle
  digests plus copied compact trust profile JSON digests across trust summaries
  at evidence aggregation and readiness replay,
  rejects all-zero trust bundle, trust pin, trust DER summary, or emitted
  profile JSON digests,
  and rejects
  plan-only, dry-run, control-bearing or whitespace-padded child-command entries,
  child-command arrays that do not start with the runner-emitted Python
  interpreter with ASCII-only numeric version suffixes plus expected stage
  script path or that carry extra positional arguments after that prefix,
  non-canonical or command-mismatched rail/notary receipt directories,
  verify commands that omit generated rail/notary receipt directories,
	  insecure-HTTP, default-profile, secret-leaking,
	  smuggled, raw-whitespace-bearing, empty/zero/leading-zero/malformed/default-port,
	  non-canonical-host, invalid-label, localhost/private-IP/rebinding-host,
	  legacy-IPv4, IPv6-transition, percent-escape, non-ASCII-path,
	  numeric-host-spoofed, or traversal-bearing URLs,
	  non-canonical canary runbook config paths, unknown upstream summary fields
	  plus live adapter, receipt, trust-bundle, and XSD JSON fields without
	  echoing control-bearing key names, secret-bearing audit-index/source
	  strings and source paths during notary publication or archived receipt
	  replay, ISO `Path.resolve()` failures during XSD manifest/schema/fixture
	  containment, trust/evidence/readiness summary input deduplication, canary
	  output/artifact/config/verify receipt corridors, rail sidecar peer,
	  duplicate receipt, message/receipt-directory overlap, and notary
	  anchor/source overlap checks, hostile pending-source-probe `Content-Type`
	  string normalization failures plus archived pending-probe `content_type`
	  and `error_kind` text normalization failures, hostile pending-probe,
	  live rail/notary adapter, receipt-verifier, canary, trust-bundle,
	  XSD-fixture, operator-evidence, and final-readiness `str` subclasses in
	  string/list, receipt-kind, compact stage-name, preview, context,
	  digest/OID, sidecar, endpoint, and secret-material replay checks,
	  recursive JSON object-key scans that now reject non-string keys and
	  normalize hostile `str` subclass keys before secret/control-field checks,
	  unknown-key validators and receipt/notary exact-key checks that reject
	  non-plain dict subclasses and use normalized present-key sets instead of
	  direct `set(value)` evaluation,
	  forbidden receipt-metadata replay checks that iterate normalized keys
	  instead of direct dict-set intersections,
	  recursive surrogate and secret-material scans that reject non-plain JSON
	  container subclasses before container method calls,
	  shared object/array and list-valued field helpers that require exact plain
	  JSON containers before semantic validation,
	  direct loaded-object checks for rail sidecars plus notary/receipt
	  persisted records, audit indexes, anchors, source sidecars, and top-level
	  receipt JSON that reject non-plain object subclasses,
	  notary/receipt status-derivation helpers that require exact plain
	  `change_reason_codes` lists before accepted-with-change classification,
	  trust-bundle public summaries, operator-evidence public canary summaries,
	  and final-readiness public summary rendering that copy only exact plain
	  JSON containers before dropping private fields or scrubbing XSD strict
	  flags, pending-probe response metadata, receipt summaries, receipt lists,
	  receipt entries, unsupported response metadata, insecure endpoints, legacy
	  message types, default-profile metadata, or non-finite public numeric
	  values,
	  rail-gateway and audit-notary HTTP status predicates that reject Python
	  boolean values before receipt metadata is digested,
	  rail/notary response-body bounding that accepts only exact built-in
	  `bytes`, `bytearray`, or `memoryview` containers before slicing or length
	  checks,
	  pending XSD probe body handling and bounded canary, receipt-verifier, and
	  xmllint pipe readers that apply the same exact bytes-like container rule,
	  repeatable direct selector inputs for pending message IDs, notary
	  endpoints, trust bundles, receipt selectors, evidence summaries, and
	  readiness summaries that require exact `list` or `tuple` containers before
	  length checks or iteration,
	  compact evidence/readiness role collectors and XSD material-path collectors
	  that require exact plain nested summary objects before `.get()` or indexing
	  can run,
	  direct scalar and repeatable path arguments that accept only sanitized
	  strings or exact concrete stdlib `pathlib` path instances before
	  filesystem loading, rejecting path subclasses before `__fspath__` or
	  `__str__` can run,
	  rail gateway direct `message` selector validation that rejects hostile
	  path-like objects and list subclasses before inbox discovery,
	  operator-evidence canary command arrays that normalize child command
	  entries to plain strings before flag, path, URL, and redaction scans,
	  all ISO operator/probe/verifier direct `main(argv=...)` normalization that
	  requires exact plain argument lists and copies hostile string subclasses
	  before CLI preflight or `argparse`, including `argv=None` rejection of
	  non-plain ambient `sys.argv` containers before slicing and explicit parser
	  program names that avoid reading ambient `sys.argv[0]`,
	  direct `run(args)` exact-`argparse.Namespace` guards before caller objects
	  can service `getattr` or `setattr`,
	  CLI-facing and JSON-summary numeric scalar helpers that require exact
	  built-in `int`/`float` values or sanitized numeric strings before
	  conversion, comparison, freshness-budget, timeout, byte-limit, count,
	  day, version, or status-code checks,
	  exact built-in integer boundaries for remaining file-read limits, bounded
	  child-command output limits, child return-code validators,
	  verified-count summaries, freshness projections, and public
	  response-metadata classifiers,
	  operator-evidence and production-readiness direct text validators that copy
	  hostile `str` subclasses to plain strings before rail-message-id, CLI
	  context/profile, artifact path, timestamp, XSD source/fixture path,
	  reviewed-gap, pending-source URL, pending-probe text, or blocked-source
	  restriction-marker checks,
	  trust-bundle source timestamp parsing that rejects non-string
	  `source.retrieved_at` values and normalizes hostile `str` subclasses before
	  canonical timestamp and freshness checks,
	  recursive unsafe-control strings,
	  synthetic-trust, record-only, or receipt-verifier-output-free evidence before
	  archival, and requires trust-summary and receipt-summary policy booleans,
	  trust profile JSON emission booleans plus a digest recomputed from archived
	  profile overrides, trust revocation booleans/counts, bundle SHA-256 values,
	  canonical sorted duplicate-free supported receipt-kind lists and compact
	  receipt entry kinds, canonical compact receipt-entry order by
	  receipt kind, path, and digest, canonical top-level canary/trust summary
	  order by compact path and digest, canonical `profile_id` order for compact
	  trust profiles,
	  per-receipt `ok=true` plus 2xx `status_code` success metadata,
	  kind-specific compact notary anchor/index/count and rail
	  message/profile/payload metadata,
	  exact direct-archive receipt digest/kind/status/endpoint-policy/metadata binding to canary summaries, no copied
	  receipt paths or digests reused across canary summaries, no relabelled
	  rail receipt `source_path`, `payload_sha256`, or `rail_message_id` reuse
	  inside one compact receipt summary, with nullable `rail_message_id` never
	  suppressing source path or payload digest replay checks,
	  and plan-only status booleans to be
	  present explicitly so omissions cannot become production defaults. Archived
	  profile overrides must also keep
  matching profile/rail/policy identities, canonical policy OIDs and CRL/OCSP
  bounded canonical base64 DER material with CRL-like or successful Basic OCSP
  response shape, material-count agreement, CRL/OCSP DER digest/byte-length
  agreement, and non-overlapping trusted/revoked pins.
  Canary summaries must also prove the runner used
	  `--require-explicit-policy`, recorded complete stdout/stderr previews for
		  every executed child stage without unsafe control characters or
		  identifier-style secret-looking material, and avoided duplicate singleton child-command
		  flags, boolean child-command flags spelled with `=value`, and non-positive,
		  non-finite, or non-canonical numeric child-command values plus Unicode digit
		  confusables in floating timeout flags, non-ASCII or non-canonical child-command path values,
		  unsupported positional command entries, wrong stage script
		  prefixes, or missing required child-command inputs. Trust-bundle preflight
		  now treats profile override emission as production-only: complete
			  source authority/version provenance is required, and `--emit-profile-json`
			  refuses local-audit record-only or insecure-source overrides plus
			  placeholder source metadata, missing source freshness budgets, or stale
			  source retrieval timestamps before writing profile JSON, then records the
			  selected `max_source_age_days` budget in the trust summary. It also
			  rejects unused local-audit `--allow-record-only`,
			  `--allow-insecure-source-url`, and `--allow-synthetic-der` flags unless
			  a verified bundle actually carries matching non-production policy,
			  insecure source URL, or synthetic DER evidence, while stripping the
			  private synthetic-DER marker from emitted summaries. The evidence gate requires
	  explicit freshness
	  budgets for canary, trust-summary, and trust-source evidence plus direct
		  receipt archive verification covering canary receipt digests and receipt
		  kinds before archival, without letting partial-canary or dry-run policy
		  hide missing direct archive receipts for non-dry-run canaries, rejects an unused
		  `--allow-plan-only` override
		  unless at least one canary summary records `plan_only=true`, rejects
		  `--allow-partial-canary` unless at least one canary summary is missing
			  a rail or notary stage, rejects unused default-profile receipt
			  overrides unless compact rail receipts actually carry missing
			  profile evidence, rejects stale retired `colr.007` receipt-summary fields,
			  and rejects unused
			  record-only/synthetic/missing-source trust overrides unless compact
			  trust summaries carry the corresponding diagnostic trust material,
			  binds compact record-only and insecure-source trust policy flags to
			  actual non-production signature policy or `http://` or local/private
			  source provenance per trust summary,
			  rejects unused dry-run, failed-receipt, insecure-HTTP, and receipt-source-missing diagnostic
			  overrides unless the archived canary command, receipt summary, or trust
			  summary carries that policy or a receipt summary records
			  `require_source_files=false`, requires failed-receipt policy to bind to
			  a failed receipt entry rather than a summary flag alone, requires
			  insecure-HTTP receipt policy to bind to compact
			  `endpoint_requires_insecure_http` evidence, requires executed
			  rail/notary child commands to carry the matching
			  `--allow-insecure-http` flag plus matching compact receipt-kind
			  endpoint evidence, requires executed rail default-profile commands
			  to carry matching compact rail receipt evidence for the same
			  diagnostic condition, requires executed
			  rail/notary stage names to match compact `receipt_kind` evidence so
			  partial canaries cannot borrow receipts from absent stages, scopes
			  verify-stage `--receipt-dir` values to the recorded rail/notary stages
			  for executed and plan-only canaries, scopes direct verify-stage
			  `--receipt` files under recorded stage receipt directories, rejects hidden endpoint
			  evidence when the summary flag is false, and binds
			  canary verify-stage
			  receipt-verifier command flags to captured receipt-verifier JSON policy
			  booleans, with production-readiness replay rejecting compact
			  failed-receipt, insecure-HTTP endpoint, stale retired `colr.007`, and
			  default-profile policy flags that contain no matching receipt entry,
			  rejects the canary-stage-only diagnostic
		  override when direct `--receipt` or `--receipt-dir` archive inputs are
		  supplied, preserves compact trust bundle SHA-256, source
		  authority/version and URL/retrieval provenance, trust source freshness
		  emission budgets, source trust-verifier diagnostic flags,
		  rejects an unused `--allow-profile-json-not-emitted` override unless
		  at least one trust summary records `profile_json_emitted=false`,
		  revoked-certificate pin
	  counts, certificate-policy OID counts,
	  CRL/OCSP material-class proof, and compact
	  trust-anchor/revoked/CRL/OCSP DER proof digests, byte lengths, and
	  cross-role uniqueness for release review, rejects profile-emittable drift and
	  emitted-but-not-emittable contradictions against the archived trust source
	  policy,
	  and the aggregate readiness gate rechecks that proof plus the evidence policy
		  context, requires explicit freshness budgets for
		  XSD/evidence/canary/trust/trust-source timestamps, blocks stale
		  digest-correct summaries and archive freshness policies weaker than the final
			  release budgets, rejects stale, placeholder, or smuggled compact trust
			  source provenance at evidence aggregation and readiness replay,
			  including separator-obfuscated `dummy`, `fake`,
			  `placeholder`, `replace-before-production`, `sample`, `template`, reserved hosts
			  such as `.example`, `example.com`, `example.net`, `example.org`, and
			  `example.invalid`, overlong URLs, invalid or overlong host labels,
			  non-ASCII host labels, numeric-host/legacy-IPv4 spoofing, IPv6 transition embedded-IPv4
			  smuggling, percent-escape smuggling, and omitted,
			  malformed, or release-weaker trust source freshness budgets, rejects
			  omitted trust-bundle source provenance separately from explicit null
			  source objects,
			  compact profile-emittable drift or emitted-but-not-emittable
		  contradictions against trust source policy or replayed trust-verifier
		  diagnostic flags, reports explicit diagnostic `source: null` compact
		  trust profiles as blockers while keeping omitted source keys malformed,
		  requires canary-stage-only evidence to record explicit
		  `receipt_verification: null` plus the matching archived
		  `allow_canary_stage_receipts_only` policy flag instead of omitting the
		  archive field or forging production policy, while still blocking that
		  policy flag when forged direct archive verification is present, rejects
		  non-boolean direct evidence-gate and readiness production-policy flags
		  before summary loading and non-boolean direct XSD strict/trust-bundle
		  policy flags before manifest or bundle loading, plus non-boolean direct
		  canary, rail-gateway, and audit-notary policy flags before config,
		  inbox, or export loading, and rejects bare strings or non-path/non-string
		  entries for direct repeatable path/endpoint arguments before trust,
		  receipt, evidence, readiness, or notary loading, caps trust bundle and
		  receipt selector path lists at 64 entries before trust/receipt loading,
		  caps recursive ISO JSON surrogate/secret-material scanners at 8192
		  array entries, 8192 object members, and 128 nesting levels before
		  walking unknown or unsupported JSON shapes, and wraps parser recursion
		  failures with the same label-only nesting diagnostic before local paths
		  or attacker-controlled leaves can be echoed,
		  caps operator-canary runbook notary endpoint and verifier
		  receipt-selector string lists at 8192 entries before entry parsing,
		  caps trust-bundle SHA-256, certificate-policy OID, and DER material
		  lists at 8192 entries before per-entry parsing,
		  caps evidence canary, trust, receipt, and receipt-directory input path
		  lists at 64 entries before evidence loading, and caps notary endpoint
		  lists at 64 values before export loading, caps untrusted XSD,
		  evidence, and readiness JSON arrays at 8192 items before semantic
		  replay, caps receipt/notary audit record, status-history, and
		  change-reason arrays at 8192 items before replay, while scalar direct
		  path arguments normalize string/path-like values and reject invalid
		  path objects before XSD, trust, canary, rail, notary, evidence, or
		  readiness loading, plus non-string direct rail Torii URLs before URL
		  validation or inbox loading, and non-string direct
		  evidence/readiness provider, environment, and default-profile context
		  values before summary loading, and now treats omitted direct
		  `argparse.Namespace` attributes for required ISO config/inbox/export
			  paths, policy booleans, evidence/readiness context strings,
			  freshness budgets, canary output limits, rail payload limits, and
			  notary response limits as controlled validation failures before file
			  discovery, summary loading, network work, or child execution, cap live
			  response-body retention at 4 MiB, treat pending-source response
			  context-exit, close, and close-accessor failures as cleanup-only
			  after bounded classification, close HTTP-error response objects
			  best-effort before archiving bounded error rows, and normalize
			  non-callable context hooks plus opener/read runtime/type/value
			  failures to `NetworkError`, and report XSD `xmllint`, canary
			  child-stage, and direct receipt-verifier startup failures, including
			  runtime/type/value launcher failures, with stable labels instead of
			  argv, local paths, or raw process-launch exception text or chained
			  traceback causes, plus their stdout/stderr
			  runtime/type/value pipe read or close failures and post-wait
			  thread `join()`/`is_alive()` bookkeeping failures as label-only
			  stage-output read errors and byte-length capping of byte-like pipe
			  chunks before preview decoding, plus runtime/type/value wait
			  failures, cleanup kill failures, and malformed return codes as
			  label-only child-finish errors,
				  and sanitize OS `strerror` text plus hostile `strerror` accessor
					  failures from input readers, input-directory checks, receipt
					  source-file checks, alias comparisons, and summary/receipt
					  writers, including reader/input directory/receipt-directory/
					  receipt-source/output/alias metadata inspection failures,
					  output parent creation failures, input
				  runtime/type/value handle failures, descriptor-close cleanup
				  failures plus temporary write/fsync/replace runtime/type/value
				  failures and cleanup unlink/close runtime/type/value failures,
				  before diagnostics so unsafe or unreadable detail collapses to
				  `I/O error`,
			  force receipt preview/error evidence to printable ASCII, while
			  trust-bundle source, operator-evidence, archived receipt, and
			  final-readiness compact timestamps must use canonical
			  `YYYY-MM-DDTHH:MM:SS[.ffffff](Z|+HH:MM|-HH:MM)` evidence
			  shape after parsing remains malformed/timezone-aware, with
			  unknown-offset `-00:00` rejected,
			  omitted optional trust source freshness budgets and rail message
		  selectors take the same defaults as their CLI forms,
		  unused final-readiness `--allow-reviewed-xsd-gaps` and
		  `--allow-canary-stage-receipts-only` overrides unless a reviewed XSD warning
		  beyond a truly unreviewed profile-version gap or canary-stage-only receipt
		  evidence is actually present, keeps truly unreviewed advertised
		  profile-version gaps as blockers even when other reviewed XSD warnings
		  exist, and
	  rechecks compact trust profile JSON emission and digest, rejects copied
	  compact trust profile JSON digests, all-zero compact receipt digests, all-zero compact
	  canary/trust summary references, and all-zero compact trust digests,
	  CRL/OCSP revocation
	  posture, direct archive/canary receipt digest/kind/status/response-body digest/endpoint-policy/metadata binding,
	  including compact replay that still reports missing archive receipt digests
	  when partial-canary or dry-run evidence policy flags are present,
	  empty successful direct-verifier stderr, trust
	  profile-count binding, and label-only missing-trust coverage blockers that
	  do not print compact receipt profile IDs or canary environment labels,
	  while rejecting repeated or copied
	  XSD/evidence and compact canary/trust summaries, rejecting nested
	  canary/trust/receipt/profile replay across evidence summaries, requiring compact
	  XSD summary/manifest/schema/fixture/blocked-source/pending-source/profile-catalog digest
	  roles to stay separate from evidence summary/receipt/canary/trust digest roles,
	  requiring compact XSD schema/fixture/blocked-source/pending-source message-definition
	  roles plus manifest/profile-catalog paths and digest roles to stay unique
	  across repeated XSD-summary inputs,
	  requiring compact XSD schema, fixture, blocked-source, and pending-source
	  arrays to remain in canonical message-definition/path or source-provenance order,
	  requiring compact
	  canary/trust summary paths and `summary_sha256` references to stay
	  role-separated, with digest references canonical nonzero digests
	  that do not reuse each other or nested receipt-summary, receipt,
	  receipt-material, profile-JSON, bundle, or DER proof digest roles inside one aggregate
	  evidence summary or across multiple evidence-summary inputs, including
	  canary-summary identities replayed as trust material and trust-summary
	  identities replayed as receipt material, requiring compact summary JSON
	  paths to stay disjoint from canary config and trust-bundle material paths
	  inside one evidence summary or across multiple evidence-summary inputs,
	  requiring compact trust bundle paths to stay unique inside and across
	  archived trust summaries, requiring compact
	  canary/trust source paths to be control-free, trim-free, not flag-shaped, and
	  traversal-free `.json` summary files, requiring compact canary runbook config paths to remain
	  traversal-free JSON pointers, requiring compact canary stage names to remain
	  unique production stages in rail/notary/verify order, rejecting raw canary
		  summaries that carry both executed and plan-only stage branches and
		  non-null successful-stage `reason` fields,
				  accepting plan-only compact summaries only with empty `stage_windows`,
				  explicitly recorded null `receipt_summary`, and canary runbook
				  planning plus planned verify commands that cover every non-dry-run
				  planned rail/notary receipt directory with unique stage receipt
				  directories, null verify-stage `receipt_dir` fields, raw plan-only
				  `dry_run` booleans that match the planned child command flags, and
				  unique, non-overlapping receipt selectors so they become production
				  blockers instead of malformed executed-evidence claims, carrying
				  compact `stage_dry_run` booleans aligned with `stage_names`, and
				  rejecting receipt kinds attached to dry-run-only rail/notary stages,
		  requiring summary digests, rejecting duplicate receipt paths or receipt digests,
	  rejecting rail/notary source path or source digest replay across canary summaries during evidence verification, rail source XML path, payload digest, or rail message-id replay within canary/archive receipt summaries when relabelled entries reuse compact source material, keeping source path and payload digest checks active when rail message ids are null, and rail/notary source-material replay across distinct evidence summaries during readiness replay,
	  rejecting notary anchor/index path or digest replay within canary/archive
	  receipt summaries during evidence verification and readiness replay while
	  still allowing repeated notary store directories for legitimate
	  multi-anchor publication,
	  rejecting non-canonical compact receipt paths and all-zero compact receipt
	  digests, compact receipt paths
	  under checked-in ISO fixture coordinates, rejecting duplicate compact
	  trust profile IDs, copied compact profile JSON digests, or bundle digests
	  across trust summaries, rejecting all-zero compact trust bundle/profile
	  JSON/DER proof digests, rejecting control-bearing or whitespace-padded
  compact identity strings, rejecting non-canonical compact trust profile IDs,
  reordered compact trust-profile arrays,
  and rejecting compact trust rail IDs outside `generic-iso20022`,
  `swift-cbpr-plus`, `fedwire-funds`, `sepa-sct-inst`, and `securities-csd`,
  rejecting unknown compact evidence fields,
  rechecking XSD manifest/profile-catalog paths and digests plus
  schema/fixture/blocked-source message-definition roles and schema/fixture
  summary arrays for count, message-definition id, digest, and cross-summary replay,
  schema-path/message-id, fixture-path segment canonicality, canonical fixture
  schema-reference strings, schema-reference consistency, and missing-schema
  fixtures that must not relabel checked-in schema message definitions as gaps,
  rejecting DTD/entity declarations before schema or fixture XML parsing,
  rejecting ambiguous schema `Document` declarations or prefixed `Document`
  type spoofing, rejecting payload `ref` indirection, weakened payload
  occurrence attributes, prefixed payload types, and missing or duplicate
  payload complex types or payload complex types without exactly one direct
  sequence, rejecting XSD composition and foreign-namespace direct children in
  schema/`Document`/payload structures, rejecting schema roots with attributes
  beyond `elementFormDefault` and `targetNamespace`, rejecting fixture
  `Document`/payload root attributes, binding parsed XML and summary digests to
  the same checked bytes, capping manifest JSON and profile catalog source at
  4 MiB and schema/fixture XML inputs at 8 MiB before parsing,
	  requiring XML schema-validation proof for every
	  schema-backed fixture while redacting local schema/fixture paths from
	  `xmllint` diagnostics, rejecting unknown XSD summary fields, recomputing
	  schema-only flags/reasons and reviewed gap-list paths/reasons from the schema/fixture
	  relationship while rejecting padded, control-bearing, non-ASCII,
	  secret-looking, or overlong reviewed reason strings plus present
	  empty/non-string archived reviewed reasons in both the XSD preflight and readiness rollup,
		  rejecting stale missing-schema reasons
			  on schema-backed archived fixtures, rejecting embedded
			  non-ASCII characters, overlong source or relative paths, overlong
			  archived XML identifiers, whitespace,
			  leading-dash path segments, semicolon path parameters, URI/drive prefixes,
			  or malformed/smuggled percent escapes in checked-in XSD source provenance,
			  rejects omitted checked-in, blocked-source, and pending-source `source` keys separately
		  from explicit null source objects,
		  manifest schema, fixture, fixture schema-reference, and archived
	  profile-catalog paths during preflight and archived-summary readiness
	  rechecks, requiring archived summaries to retain the emitted manifest
	  path and explicit profile-catalog object/null state, binding archived schema
	  namespaces, fixture schema message ids, and fixture payload roots back to
	  their referenced schemas, requiring
	  profile-catalog source and embedded JSON
  digest provenance from exactly one active Rust `DEFAULT_PROFILES_JSON`
	  raw-string declaration, rejecting archived manifest digests that reuse
	  schema, fixture, blocked-source, pending-source, profile-catalog source, or profile-catalog
	  JSON digest roles, plus duplicate-free profile/message/direction/version shape
	  and canonical skipped family-version aliases
	  with unknown source catalog keys rejected by the XSD preflight and
	  runtime catalog-field shapes checked before summary emission, requiring
	  profile-catalog enum and list values to remain printable ASCII before
	  unknown-value diagnostics or summary recording, rechecking
	  canonical profile ids, ISO family message types, allowed directions, and
  message-definition family binding in consumed summaries, and schema-backed
  proof for advertised concrete message versions, recomputing
  profile-catalog missing-version lists and represented profile-id counts, and
  requiring timezone-aware non-future XSD/evidence/trust verification
  timestamps and ordered canary and non-overlapping per-stage start/finish
  windows for final evidence traceability. Compact stage-window names must
  match the recorded stage sequence, and compact stage names are rechecked as
  unique production rail/notary/verify stages. Compact canary/trust summary
  paths, canary config paths, receipt paths, and child receipt-directory
  arguments reject embedded whitespace, leading-dash path segments, semicolon
  path parameters, empty segments, raw backslashes, and traversal segments before aggregation. The repository also
  carries plan-valid templates for Swift CBPR+, Fedwire Funds, SEPA SCT Inst,
  and securities CSD operator canaries. Remaining persistence work is
  provider-specific live service canaries and vendor evidence that passes the
  aggregate production-readiness gate.
	  ISO rail ingress now has
	  an operator file-drop adapter that verifies sidecar-pinned message
		  type/profile/payload digests, rejects unsupported or non-ASCII rail
		  message types, non-canonical sidecar profile IDs, and non-canonical
		  rail-message IDs, and closes the live sidecar schema while bounding sidecar JSON before
	  submitting to clean Torii base URLs and writing receipts, plus the same
	  receipt verifier and runbook runner for canary evidence.
  Remaining rail-connectivity work is provider-specific live gateway canaries
  and archived rail evidence. Live
  securities lifecycle profile admission now checks
  local ISIN/CUSIP, MIC, BIC/LEI, CSD venue, settlement-account, and cash-leg
  snapshots before durable `sese.023` recording; remaining securities work is
  live-rail adapter coverage around production CSD/account/cash-leg sources.
  `require-verified` profiles now require profile-specific public-key pins or
  linked terminal CA trust-anchor DER SHA-256 pins before a P-256/SHA-256
  enveloped signature can pass, with deterministic leaf/issuer
  distinguished-name binding, non-CA leaf enforcement, critical leaf
  `keyUsage`/`digitalSignature`, critical issuer CA basicConstraints, critical
  issuer `keyUsage`/`keyCertSign`, issuer path-length constraint enforcement,
  required certificate-policy continuity through intermediate CAs below the
  terminal trust anchor, fail-closed rejection of policy mappings, policy
  constraints, and inhibit-any-policy extensions,
  bounded duplicate-free `X509Data` chains, certificate-chain
  ECDSA-with-SHA256/secp256r1 enforcement with uncompressed P-256 SEC1 SPKI
  bytes, unsupported-critical-extension rejection, and validity-at-signing checks
  for X.509 chains, explicit certificate revocation pins, configured and
  signature-scoped embedded CRL/OCSP signer revocation checks evaluated against verified XAdES
  `SigningTime` or BAH `CreDt` rather than local wall clock, plus an offline
  trust-bundle verifier with semantic DER-shape checks, required clean
  provenance URL and retrieval-time fields, trim-free source
  authority/version provenance for archives, duplicate-label rejection,
  DER-object digest keys
  that fail closed when present as `null` or another non-string value, omitted
  absent labels in trust summaries, archived-summary `label: null` rejection,
  repeated-separator URL path rejection, and
  repeated-path/copied-bundle/duplicate-profile rejection, canonical lowercase
  trust profile ID enforcement, known ISO rail ID enforcement, plus
  profile-family templates for operator PKI preflight. The templates are
  schema/CI scaffolding
  only, require an explicit synthetic-template flag, and cannot emit profile
  overrides; remaining trust work is replacing them with official rail packages
  and archiving live provenance evidence that passes the production evidence
  gate. Canonical
  lowercase SHA-256 trust/revocation pin admission, shortest-form DER
  length/minimal positive-integer admission for parsed OCSP responses,
  fail-closed rejection of unsupported OCSP response/single-response extensions, low-S
  fixed-width P-256 `r || s` or low-S DER ECDSA signature-value decoding, and a
  deterministic supported canonical XML subset that expands empty elements,
  normalizes attribute quotes, and sorts namespace
  declarations plus unprefixed, declared prefixed, and implicit `xml:` namespace
  attributes while omitting the fixed legal `xmlns:xml` declaration from
  canonical output, decoding predefined/numeric XML character references and
  applying root namespace declarations inherited from an enclosing XMLDSig `Signature`
  according to the declared C14N mode: all inherited root declarations for
  inclusive C14N and only visibly used inherited root declarations for exclusive
  C14N. Non-empty same-document payload References must strictly enclose the
  verified signature carrier so partial subtree signatures cannot authenticate
  unsigned payment fields. Payload References may now add one final supported
  C14N transform after the required enveloped-signature transform to drive
  digest canonicalization.
  XMLDSig method and transform elements remain parameter-free and fail closed on
  non-whitespace child content such as `InclusiveNamespaces`, XPath, HMAC, or
  digest parameters; critical method elements must appear exactly once, Reference
  transforms must be enclosed in exactly one attribute-free `Transforms` wrapper,
  only implemented ordinary attributes are accepted (`Algorithm`, payload
  Reference `URI`, and XAdES Reference `URI`/`Type`), those policy attributes
  are read by exact XML attribute name only, and supported Reference
  children must remain ordered as `Transforms`, `DigestMethod`, then
  `DigestValue`; top-level `Signature` and `SignedInfo` children must also stay
  in the supported XMLDSig order. Payloads may contain exactly one supported
  signature carrier: either a bare XMLDSig `Signature` or an ISO `Sgntr` wrapper
  with exactly one direct XMLDSig `Signature` child. Any additional
  `Signature`/`Sgntr` element outside the verified carrier fails closed.
  Prefixed XMLDSig structural elements must
  resolve to the XMLDSig namespace across the supported Signature, SignedInfo,
  Reference, digest/transform, and KeyInfo subtrees, and supported XML element
  spans require exact qualified-name matches between opening and closing tags.
  Selected structural QNames must also pass the supported XML name policy, so
  malformed local-name matches such as double-colon XMLDSig tags fail closed
  before namespace, child-shape, digest, or signature handling continues.
  Unprefixed XMLDSig/XAdES structural elements reject explicit default
  namespaces that conflict with the supported XMLDSig or XAdES namespace.
  Required base64 values are singleton attribute-free text leaves without nested
  markup or comments; `PublicKey`/`X509Certificate` credential leaves follow the
  same no-markup rule. Public-key material cannot be mixed with
  `X509Certificate` material in one `KeyInfo`; key material must be scoped to
  exactly one structured `KeyInfo` using either `KeyValue/ECKeyValue` with
  P-256 `NamedCurve` whose `PublicKey` bytes parse as an uncompressed P-256
  SEC1 point, or one bounded duplicate-free `X509Data` wrapper, with
  unsupported direct child elements, unsupported ordinary attributes, and
  non-whitespace wrapper text rejected. The XAdES `SigningCertificateV2` subset
  uses a non-empty,
  duplicate-free ordered prefix of the verified certificate-chain digests with
  attribute-free direct `Cert`/`CertDigest` wrappers, `DigestMethod` with only
  `Algorithm`, and singleton attribute-free signed `SigningTime` text; prefixed
  XAdES structural elements must resolve to the ETSI XAdES v1.3.2 namespace.
  It still fails closed for inherited namespace context beyond root
  declarations, unbound prefixed attributes, reserved namespace rebindings,
  CDATA/CDEnd tokens, uppercase `#X` numeric character references,
  DTD/general/custom entity expansion, and all other XML outside the
  implemented subset.

