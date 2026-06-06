# TradFi ISO 20022 Interop Audit

This audit describes the repository-level ISO 20022 and TradFi interoperability
surface. It is a design and implementation map for broad interop readiness; it
does not claim direct live SWIFT, Fedwire, SEPA, or CSD network connectivity.

## Standards Baseline

- ISO 20022 message definitions, MDRs, schemas, examples, Business Application
  Header material, external code sets, and SupplementaryData material remain the
  canonical source of truth for message layout and versioning.[^iso_catalogue]
- Swift CBPR+ is now ISO 20022-first: Swift reports that the MT and ISO 20022
  coexistence period for cross-border payments and reporting ended on
  2025-11-22, after which ISO 20022 is required for payment instructions on that
  rail.[^swift_cbpr]
- Fedwire Funds completed its ISO 20022 migration on 2025-07-15, so any Fedwire
  profile must be strict about the selected message version, business service,
  participant identifiers, and USD minor units.[^fedwire_iso]
- CPMI harmonisation guidance is maintained through at least end-2027; the bridge
  should treat those requirements as profile input for cross-border payment
  data quality, not as a runtime network connection.[^cpmi_harmonisation]

## Current Repository Surface

- `ivm::iso20022` contains deterministic parsers and static schemas for payment,
  securities, cash-management, and collateral message families.
- Torii exposes `pacs.008` and `pacs.009` ingestion plus lifecycle/status
  submission for payment, securities, and collateral follow-up messages. The
  bridge builds signed transfer transactions from configured account aliases,
  currency bindings, and reference-data snapshots, and records non-ledger
  lifecycle messages durably for audit.
- The JavaScript SDK contains builders for `pacs.*` and `camt.*` payloads and
  client helpers for Torii ISO submissions and polling.
- The CLI can emit `sese.023` and `sese.025` settlement previews for DvP/PvP
  flows, with optional reference-data validation for delivery instrument IDs.
- Reference-data loaders ingest ISIN/CUSIP, BIC/LEI, and MIC snapshots from
  operator-provided files. No runtime remote fetch is performed.

## Implemented In This Pass

- Added a shared static profile catalog in `iroha_core::iso_bridge::profiles`
  for:
  - `generic-iso20022`
  - `swift-cbpr-plus`
  - `fedwire-funds`
  - `sepa-sct-inst`
  - `securities-csd`
- Extended `iso_bridge` config with:
  - `default_profile`
  - `profiles`
  - `store_dir`
  - `embedded_signature_policy`
  - per-profile required reference datasets and message profiles
- Made Torii inbound validation profile-aware for existing `pacs.008` and
  `pacs.009` endpoints:
  - profile selected by `X-Iroha-Iso-Profile`, then `?profile=...`, then config
    default
  - message definition version and business service checks
  - Business Application Header checks for live profiles
  - UETR capture and replay detection
  - profile-required reference-data gates
  - amount minor-unit checks
  - structured address and SupplementaryData limits
  - embedded XMLDSig/XAdES markers recorded for generic ISO, rejected for live
    profiles that do not enable verification, and accepted for
    `require-verified` profiles only after P-256/SHA-256 verification plus
    profile-specific public-key, leaf-certificate, or linked certificate-chain
    SHA-256 pin matching with uncompressed P-256 SEC1 raw public-key
    validation, leaf `digitalSignature`, and issuer critical
    CA `basicConstraints`/`keyCertSign` policy checks, certificate-chain
    ECDSA-with-SHA256/secp256r1 plus uncompressed P-256 SEC1 SPKI enforcement,
    child issuer distinguished-name binding to parent subject distinguished
    names, issuer path-length constraint enforcement, unsupported-critical-extension
    rejection, and validity-at-signing checks using verified signed XAdES
    `SigningTime` or BAH `CreDt`. Unsigned
    signature-local `SigningTime` values are not trusted. ECDSA `SignatureValue`
    accepts fixed-width P-256 `r || s` and DER encodings only when `s` is
    canonical low-S.
    Reference URIs are limited to one empty URI or one unique same-document `#id`
    target that strictly encloses the verified signature carrier, with an
    enveloped-signature transform first, at most one final
    supported C14N transform that controls digest canonicalization, and SHA-256
    digest method. One optional XAdES `SignedProperties` Reference may target a
    local `#id` with the XAdES `SignedProperties` Type URI, exactly one supported
    C14N transform, and a SHA-256 digest; its enclosing `QualifyingProperties`
    target must bind to the enclosing `Signature` `Id`. Certificate-backed XAdES
    signatures must present a non-empty, duplicate-free ordered prefix of the
    verified XMLDSig certificate-chain SHA-256 digests, starting with the leaf
    certificate. The supported signed-property subset requires direct
    `Signature/Object/QualifyingProperties/SignedProperties/SignedSignatureProperties`
    structure; `QualifyingProperties` accepts only `Target`,
    `SigningCertificateV2` accepts only attribute-free direct `Cert` children
    with attribute-free direct `CertDigest` children, a `DigestMethod` carrying
    only `Algorithm`, and text-only digest values; signed `SigningTime` is a
    singleton attribute-free text leaf. Prefixed XAdES structural elements must
    resolve to the ETSI XAdES v1.3.2 namespace, and unprefixed XMLDSig/XAdES
    structural elements reject explicit conflicting default namespaces. Any
    `SignedProperties` element under the signature must be the verified
    referenced direct target; unreferenced, wrapped, or duplicate
    `SignedProperties` elements fail closed.
    Same-document targets carry ancestor namespace
    declarations into root canonicalization, constrained to a fail-closed
    supported canonical XML subset for empty-element expansion, attribute quote normalization, namespace
    declarations plus unprefixed, declared prefixed, and implicit `xml:`
    attribute sorting, legal `xmlns:xml` declaration omission, and
    predefined/numeric XML character-reference decoding.
    Supported XMLDSig method and transform elements are parameter-free and reject
    non-whitespace child content such as `InclusiveNamespaces`, XPath, HMAC, or
    digest parameters. Method, transform, digest, and Reference policy
    attributes are read by exact XML attribute name only, so namespace-qualified
    spoof attributes fail closed. Critical method elements must appear exactly once, and
    Reference transforms must be enclosed in exactly one attribute-free
    `Transforms` wrapper; only implemented ordinary attributes are accepted
    (`Algorithm`, payload Reference `URI`, and XAdES Reference `URI`/`Type`).
    Extra direct children under `Reference` or `Transforms` fail closed, and
    supported References must keep direct children ordered as `Transforms`,
    `DigestMethod`, then `DigestValue`.
    Top-level `Signature` and `SignedInfo` parsing accepts only implemented
    direct children in supported XMLDSig order, so reordered or wrapped
    `SignedInfo` or method nodes, unsupported direct children, and duplicate
    singleton signature nodes fail closed. The payload may contain exactly one
    supported signature carrier: either a bare XMLDSig `Signature` or an ISO
    `Sgntr` wrapper with exactly one direct XMLDSig `Signature` child. Any
    additional `Signature`/`Sgntr` element outside the verified carrier fails closed.
    Prefixed XMLDSig structural elements
    must resolve to the XMLDSig namespace in their inherited scope across
    `Signature`, `SignedInfo`, Reference transforms/digests, and public-key or
    X.509 `KeyInfo` material. Supported XML element spans require exact
    qualified-name matches between opening and closing tags.
    Required XMLDSig base64 fields reject duplicates and must be attribute-free
    text leaves without nested markup or comments, and `PublicKey`/
    `X509Certificate` credential leaves follow the same no-markup rule.
    Public-key material cannot be mixed with `X509Certificate` material in one `KeyInfo`; key
    material must be scoped to exactly one structured `KeyInfo` using either
    `KeyValue/ECKeyValue` with P-256 `NamedCurve` or one `X509Data` wrapper,
    with unsupported direct child elements, unsupported ordinary attributes, and
    non-whitespace wrapper text rejected.
    Valid XML comments are omitted for the supported no-comments C14N algorithms.
    Root namespace declarations inherited from the enclosing `Signature` are
    applied by C14N mode: all inherited root declarations for inclusive C14N and
    only visibly used inherited root declarations for exclusive C14N.
    `revoked_certificate_sha256` pins deny otherwise trusted chains
- Added durable ISO bridge state under `store_dir/messages/*.json`, including
  payload hash, profile metadata, UETR, transaction hash, status history, reason
  codes, context, reference snapshot id, and a deterministic `record_sha256`
  digest that binds the persisted JSON body.
- Added a deterministic local audit index at
  `store_dir/audit/messages.index.json`; it lists sorted durable message
  entries, links each entry to the message file's `record_sha256`, and carries
  its own `index_sha256` digest for external archival or notarization. Torii now
  serves the same deterministic manifest at
  `GET /v1/iso20022/audit/messages` after the normal access checks.
- Added operator-controlled durable-store retention with
  `store_retention_secs` and `store_max_records`. Both default to `0` to retain
  all durable records; when configured, compaction removes expired or oldest
  overflow records, clears replay indexes, deletes the corresponding
  `store_dir/messages/*.json` files, and regenerates the audit index from
  survivors.
- Added `audit_export_dir`, an operator-configured external audit spool. When
  durable persistence regenerates the audit index, Torii mirrors
  `messages.index.json` into that directory and writes a digest-addressed
  `anchors/{index_sha256}.notary.json` preimage plus `latest.notary.json`, each
  binding the exported `index_sha256`, record count, source `store_dir`, and
  embedded audit manifest with its own `anchor_sha256`.
- Added `scripts/iso_audit_notary_adapter.py`, an operator-side archival/notary
  adapter for the exported preimages. It verifies the anchor digest, embedded
  audit-index digest, top-level `index_sha256`, digest-addressed anchor
  filename, duplicate-free audit records, local `messages.index.json` equality,
  and record-count consistency before posting to HTTPS endpoints, requires
  non-empty anchors to expose
  `store_dir/messages` record sources by default, and verifies each indexed
  persisted record body against its `record_sha256`, row metadata, and monotonic
  current status history before network delivery, while anchor `store_dir`
  values reject whitespace, leading dashes, leading-dash path segments,
  backslashes, semicolon path parameters, empty path segments, and dot/parent
  path segments; local
  HTTP is rejected unless explicitly
  enabled for tests, endpoint URLs must not contain credentials, params, query
  strings, fragments, surrounding or embedded whitespace, or control
  characters, and empty/zero/leading-zero/malformed/default ports plus non-canonical hosts are
  rejected along with invalid DNS labels, percent-escaped hosts, numeric-host
  spoofing, traversal, encoded-separator, encoded-semicolon, encoded URL
  delimiters, encoded-percent, percent-encoded
  control/space bytes, malformed percent escapes, backslash, and embedded
  semicolon path smuggling, duplicate publication endpoints are rejected before
  network delivery, remote redirects are not followed and are archived as failed
  receipts, bearer-token files must be regular non-symlink inputs
  capped at 8 KiB before decoding to exact UTF-8 with no surrounding
  whitespace, embedded whitespace, or control characters, and
  the export directory plus export inputs (`latest.notary.json`, the
  digest-addressed anchor peer, `messages.index.json`, and
  clean `store_dir/messages` record sources) must be non-symlink regular
  directories/files. The `--export-dir` and `--bearer-token-file` CLI paths
  reject raw control characters, whitespace, leading-dash segments,
  backslashes, semicolon parameters, empty segments, and dot/parent traversal
  before argparse `Path` normalization, `--timeout-secs` must be positive and
  finite, and `--response-limit-bytes` must be a positive integer. Each JSON
  input is capped at 64 MiB before every publish attempt writes a bounded
  receipt without persisting bearer-token material, redacting secret-looking
  remote response previews or transport errors before persistence. Rejected
  endpoint URL validation errors report the structural failure by field label
  without echoing raw URL strings that may contain query secrets.
  Receipt output directories and receipt leaves are preflighted before
  publication, reject control characters, whitespace, leading-dash segments,
  backslashes, semicolon parameters, empty segments, dot/parent traversal,
  symlinked existing ancestors, and hard-linked, symlink, or non-regular
  targets, and receipts are written via exclusive same-directory
  owner-private temporary files with bounded digest-derived names that are
  descriptor-rechecked, fsynced, and atomically replaced where the platform
  supports it.
- Hardened durable ISO bridge reload so missing, malformed, or mismatched
  persisted-record digests fail closed and do not rebuild message status or
  replay indexes.
- Added richer JSON status fields to existing responses and exposed
  `/v1/iso20022/messages/{msg_id}` as an alias for message records.
- Added `/v1/iso20022/messages/{msg_id}/pacs002` to emit a current `pacs.002`
  status report XML from the same status record.
- Added `/v1/iso20022/colr012` to record `colr.012` collateral substitution
  confirmations with durable obligation, original/substitute collateral,
  effective-date, substitution-type, haircut, and reason-code context. The
  legacy `colr.007` parser/route remains for local compatibility, but the
  default profile catalog and production XSD manifest use the ISO
  collateral-substitution confirmation family `colr.012.001.05`.
- Hardened live securities profile admission so `sese.023`/`sese.025` reference
  fields fail closed when the loaded reference snapshots do not contain the
  settlement instrument ISIN/CUSIP, an active place-of-settlement MIC, or mapped
  delivering/receiving party BICs.
- Added securities ledger crosswalk snapshots for CSD venues, CSD settlement
  accounts, and securities cash legs. Live `securities-csd` `sese.023`
  admission now requires a mapped instrument asset, CSD ledger domain,
  delivering and receiving settlement accounts, and cash-leg asset definition
  before durable lifecycle recording.
- Added a checked-in `sese.024` securities status-advice XML fixture and Torii
  regressions covering known-original pending updates, unknown originals,
  wrong-family originals, and conflicting settlement references.
- Added checked-in `pacs.004` and `camt.056` XML fixtures and Torii lifecycle
  regressions proving payment returns reject known originals and cancellation
  requests put known originals on hold without fabricating missing originals.
- Added a checked-in `pacs.002` XML fixture and Torii lifecycle regression
  proving an accepted status report settles a known original payment. Additional
  adversarial coverage pins `GrpHdr/MsgId` as the durable status-report id so
  `TxInfAndSts/StsId` cannot shadow lifecycle or audit identity.
- Added full Standards Editor XSD fixtures for `pacs.002.001.10`,
  `pacs.004.001.10`, `camt.056.001.08`, and `camt.056.001.09` to the offline
  MDR-derived schema set. The live-profile fixture matrix now admits BAH status,
  return, and cancellation reports under the configured
  version/business-service controls for the rails that allow those exact
  versions.
- Added `fixtures/iso20022/xsd/fixture_manifest.json` plus
  `scripts/iso_xsd_fixture_verify.py`, an offline manifest preflight that binds
  checked-in XSDs to checked-in XML fixtures, verifies schema target namespaces
  and unambiguous `Document` payload roots with exactly one top-level
  `Document` element whose type is exactly the local `Document` type, one
  referenced `Document` complex type, one direct `Document` sequence, and one
  direct payload element with exact `name`/`type` attributes, no `ref`
  indirection, a local unprefixed type, and exactly one matching local payload
  complex type containing exactly one direct `xs:sequence`, rejects XSD
  composition (`xs:import`, `xs:include`, `xs:redefine`, `xs:override`) and
  foreign-namespace direct children under the schema, `Document`, and payload
  structures, and requires schema roots to declare exactly
  `elementFormDefault` and `targetNamespace` without external schema-location
  hints or other root attributes,
  verifies XML fixture namespaces and payload
  roots, requires fixture `Document` and immediate payload roots to be
  attribute-free, rejects DTD/entity declarations before parsing schemas or XML fixtures,
  parses and hashes each manifest/schema/fixture input from the same checked
  byte buffer, caps manifest JSON and profile catalog source at 4 MiB and
  schema/fixture XML inputs at 8 MiB before parsing, drains `xmllint`
  stdout/stderr through a 64 KiB cap and bounds validator runtime with positive
  finite `--xmllint-timeout-secs` during optional schema validation,
  enforces canonical lowercase ISO message definition ids and path
  containment for schema/fixture entries, rejects manifest schema paths,
  fixture paths, and fixture schema references with backslashes, embedded
  whitespace, leading-dash path segments, semicolon path parameters, empty segments, dot segments, or
  forbidden parent segments, rejects copied XML fixtures with duplicate
  fixture SHA-256 values, optionally validates schema-backed XML fixtures
  against their checked-in XSDs with `xmllint --nonet`, optionally
  parses the embedded default rail profile catalog and records which concrete
  advertised message versions are schema-backed, emits digest-bound summaries,
  records manifest, schema, fixture, profile source-file, and embedded catalog
  JSON SHA-256 provenance, requires exactly one active Rust
  `DEFAULT_PROFILES_JSON` raw-string declaration and ignores spoofed matches in
  comments or unrelated strings, requires each checked-in XSD to carry canonical
  repository, commit, source path, SPDX license, and source SHA-256 provenance
  that matches the checked-in bytes, caps source repository URLs at 2048
  characters, rejects source provenance paths with
  embedded whitespace, leading-dash path segments, or semicolon path parameters, rejects XSDs that contain
  known restricted Standards Editor redistribution terms, rejects duplicate, malformed, or
  unknown-key profile catalog profile/message/direction/version entries, and
  allows only the exact message-family alias or concrete message definition
  ids in catalog `versions` lists so mistyped aliases cannot bypass
  schema-backed checks, rejects control-bearing manifest/profile-catalog strings
  before summary emission, and validates optional runtime catalog fields when
  present rather than accepting explicit `null`: runtime-required rails, embedded-signature policies, and
  structured-address modes; optional trust/revocation pins; required reference
  datasets; booleans; supplementary-data caps; business-service requirements;
  and amount minor-unit rows. Current and legacy trust-pin aliases cannot
  overlap, trusted and revoked certificate pins cannot overlap, CRL/OCSP
  material must be bounded canonical base64 containing complete DER SEQUENCE
  envelopes, revocation flags must carry corresponding material, and
  `require-verified` catalog profiles must carry at least one public-key or
  X.509 trust pin. It also makes missing-XSD fixture coverage explicit.
  All
  checked-in XSDs now have standalone XML fixtures and pass
  `--validate-xml-schema`, so `--require-fixture-for-schema` passes; the
  schema-backed strict flags still intentionally fail until the remaining
  official profile-advertised payment, securities, and collateral XSD packages
  are checked in.
- Updated OpenAPI and MCP submission surfaces to expose profile selection.
- Added `scripts/iso_rail_gateway_adapter.py`, an operator-side file-drop
  adapter for live rail gateway ingress. Each inbound `*.xml` must have a
  sibling `*.xml.json` sidecar that pins `message_type`, `profile`, and
  `payload_sha256`; the adapter verifies those values before posting to the
  matching Torii ISO endpoint, rejects plaintext HTTP unless explicitly enabled
  for local tests, rejects Torii base URLs with credentials, params, query
  strings, fragments, surrounding or embedded whitespace, or control
  characters, overlong URLs or DNS hosts, localhost/local-private IP literals,
  known local/private rebinding hostnames, or IPv6 transition addresses embedding
  non-global IPv4 addresses, empty/zero/leading-zero/malformed/default ports,
  non-canonical hosts, invalid host labels
  or numeric-host/legacy-IPv4 spoofing, percent-encoded control/space bytes, malformed
  percent escapes, and smuggled URL paths including encoded semicolon parameters,
  encoded URL delimiters, and repeated separators, keeps
  explicit `--message` paths inside the declared inbox, requires an explicit
  profile by default, rejects duplicate payload digests or duplicate
  `rail_message_id` values within one gateway run before network delivery,
  rejects sidecar `profile` and `rail_message_id` values
  that are explicitly `null` or carry surrounding whitespace, embedded
  whitespace, or control characters, rejects non-canonical sidecar profile IDs,
  rejects sidecar `rail_message_id` values that are longer than 128 characters
  or are not canonical ASCII rail-message identifiers, rejects unknown sidecar
  fields, bounds sidecar JSON before parsing,
  rejects legacy `colr.007`
  collateral drops unless `--allow-legacy-colr007` is set for local diagnostics,
  requires bearer-token files to be regular non-symlink inputs capped at 8 KiB
  before decoding to exact UTF-8 with no surrounding whitespace, embedded
  whitespace, or control characters, rejects raw `--inbox-dir` and
  `--bearer-token-file` CLI path smuggling before argparse `Path`
  normalization, requires positive finite `--timeout-secs`, and requires
  positive integer `--max-payload-bytes` and `--response-limit-bytes`,
  does not follow remote redirects, and writes bounded local receipts for
  successful and failed submissions
  without persisting token material, redacting secret-looking remote response
  previews or transport errors before persistence. Rejected Torii URL
  validation errors report the structural failure by field label without
  echoing raw URL strings that may contain query secrets. XML payload and sidecar inputs must be
  real files in the drop area and are read through bounded file caps:
  symlinked XML payloads or sidecar JSON files are rejected before network
  delivery. Explicit `--message` paths and discovered XML leaves reject
  whitespace, leading dashes, leading-dash path segments, backslashes,
  semicolon path parameters, empty path segments, and dot/parent path segments
  before sidecar or payload reads. Receipt output
  directories and receipt leaves are preflighted before
  Torii submission, reject control characters, whitespace, leading-dash
  segments, backslashes, semicolon parameters, empty segments, dot/parent
  traversal, symlinked existing ancestors, and hard-linked, symlink, or
  non-regular targets, and are written via exclusive same-directory
  owner-private temporary files with bounded digest-derived names that are
  descriptor-rechecked, fsynced, and atomically replaced where available. The
  inbox directory and both
  discovered and explicit `--message` XML leaves preserve symlink boundaries so
  symlinked gateway inputs fail before network delivery.
- Added `scripts/iso_operator_receipt_verify.py`, a read-only canary verifier
  for rail/notary adapter receipts. It recomputes receipt digests, requires
  successful 2xx receipts by default, rejects plaintext HTTP evidence unless
  explicitly enabled for local tests, rejects receipt endpoint URLs with
  credentials, params, query strings, fragments, malformed hosts, surrounding
  or embedded whitespace, control characters, empty/zero/leading-zero/malformed/default ports, or
  non-canonical host spelling, localhost/local-private IP literals, known
  local/private rebinding hostnames, IPv6 transition addresses embedding
  non-global IPv4 addresses, invalid host labels, percent-escaped hosts,
  numeric-host/legacy-IPv4 spoofing, plus path traversal, encoded path separators,
  repeated path separators, encoded-semicolon parameters, encoded URL
  delimiters, encoded-percent path segments, or percent-encoded control/space bytes,
  detects leaked authorization/token
  material, reports rejected endpoint URL structure without echoing raw URL
  strings that may contain query secrets, closes the raw receipt schemas for each receipt kind plus notary
  anchor/audit-index source schemas, including duplicate-free nested audit records, binds
  audit record filenames to `sha256(message_id).json`, binds each indexed
  `record_sha256` to the persisted `store_dir/messages` body when source files
  are required or locally available, rejects row/source metadata drift and
  persisted-state-derived `pacs002_code` or status-history timestamp drift,
  binds recorded endpoint digests to the recorded endpoint URLs, requires
  timezone-aware adapter timestamps that do not require trimming, enforces
  `ok`/`status_code` consistency, validates bounded response metadata, can
  cross-check referenced XML, rail sidecar, or notary-anchor source files,
  requires rail `xml_path` values to point at `.xml` leaves and rail sidecars
  to match the adapter's `xml_path + .json` convention,
  requires notary `anchor_path` values to keep the `latest.notary.json` or
  digest-addressed `anchors/<index_sha256>.notary.json` shape even when source
  files are not required,
  rejects raw notary `anchor_path` and `store_dir` values, raw rail receipt
  `message_type`, `xml_path`, and `sidecar_path` values that carry whitespace,
  control characters, leading dashes, leading-dash path segments, backslashes,
  semicolon path parameters, empty path segments, or dot/parent path segments,
  plus receipt and source-sidecar rail
  `profile`/`rail_message_id` values when they carry surrounding whitespace or
  embedded whitespace or control characters, rejects non-canonical receipt or
  source-sidecar profile IDs, rejects overlong or non-canonical ASCII
  `rail_message_id` values, caps receipt JSON at 4 MiB, notary source JSON at
  64 MiB, rail source XML at 4 MiB, and source-sidecar JSON at 16 KiB before
  parsing or hashing, replays
  digest-addressed notary-anchor and
  `messages.index.json` checks while rejecting symlinked or non-regular notary
  anchor/index peers and rail XML/sidecar files, rejects legacy `colr.007` rail
  source files unless `--allow-legacy-colr007` is set for local diagnostics,
  rejects symlinked receipt archive directories before discovery, and rejects
  repeated receipt paths or copied receipts with duplicate `receipt_sha256`
  values before emitting a
  digest-bound verifier summary with per-receipt file paths, `receipt_sha256`
  values, and policy flags.
- Added `scripts/iso_operator_canary.py`, a strict JSON-runbook runner that
  executes the rail file-drop adapter, audit notary adapter, and receipt
  verifier as subprocesses. It caps runbook JSON at 64 KiB before parsing,
  requires explicit provider/environment labels,
  rejects unknown runbook keys, rejects surrounding whitespace and control
  characters in runbook strings, rejects present `null` optional path and
  numeric limit fields instead of silently applying defaults, keeps relative
  paths inside the runbook directory, rejects endpoint URLs with credentials,
  params, query strings, fragments, malformed bracketed hosts, overlong URL
  strings, DNS hosts longer than 253 characters, localhost/local-private IP
  literals, known local/private rebinding hostnames, or legacy IPv4 numeric
  notation, or IPv6 transition addresses embedding non-global IPv4 addresses,
  rejects duplicate endpoint lists and duplicate receipt inputs,
  redacts bearer-token file arguments in its summary, verifies generated
  receipts by default, bounds each child stage with positive finite
  `--stage-timeout-secs`, records `timed_out` for killed children, drains child
  stdout/stderr through a bounded preview cap instead of retaining unbounded
  output, and writes a single bounded summary JSON for operator evidence archives. Summary output paths are preflighted
  before subprocess stages, reject control characters, whitespace, leading-dash
  segments, backslashes, semicolon parameters, empty segments, dot/parent
  traversal, symlinked existing ancestors, and hard-linked, symlink, or
  non-regular targets, and are written via exclusive same-directory
  owner-private temporary files with bounded digest-derived names that are
  descriptor-rechecked, fsynced, and atomically replaced where available.
  `--plan-only`
  validates runbooks and prints redacted child commands without contacting Torii
  or notary endpoints.
- The ISO operator scripts reject duplicate JSON object keys, non-standard
  `NaN`/`Infinity` JSON constants, and lone UTF-16 surrogate escapes before
  semantic validation across rail sidecars, notary anchors/indexes/record
  sources, canary runbooks, receipts, trust bundles, XSD manifests/profile
  catalogs, evidence summaries, readiness summaries, receipt-verifier JSON
  embedded in canary stdout, and direct archive receipt-verifier stdout. This
  prevents shadowed keys, non-finite numeric values, or invalid Unicode strings
  from changing the release-evidence meaning after digest or policy checks.
  Canary runbooks,
  trust bundles, evidence/readiness summaries, XSD manifests, profile catalogs,
  schema files, XML fixtures, and receipt archive directories must reject
  symlinked existing ancestors plus symlink or non-regular leaves before digest,
  provenance, discovery, or policy checks run; checked file inputs are opened
  through no-follow file descriptors where available so the read uses the same
  regular file that was checked. Direct CLI artifact flags for live rail inbox
  roots, live notary export roots, rail/notary bearer-token files, canary
  configs, trust bundles, XSD manifests/profile catalogs, receipt
  files/directories, canary/trust summaries, and XSD/evidence summaries also reject raw control
  characters, whitespace, leading-dash segments, backslashes, semicolon
  parameters, empty segments, and dot/parent traversal before argparse `Path`
  normalization or file discovery. Live rail/notary adapter timeouts must be
  positive finite numbers, and their response/payload byte caps must be positive
  integers before any local read or network delivery. Operator receipt, summary,
  and emitted profile-override output paths also reject control characters, whitespace,
  leading-dash segments, backslashes, semicolon parameters, dot/parent
  traversal, empty segments, symlinked existing ancestors, and hard-linked,
  symlink, or non-regular targets before writing, then replace the target
  atomically from an owner-private descriptor-checked temporary file with a bounded
  digest-derived name.
  Canary runbook paths reject embedded whitespace, leading-dash path segments,
  backslashes, semicolon path parameters, empty segments, and dot/parent
  traversal segments before path expansion or child-script planning.
  XSD manifest/profile optional fields are optional only when omitted; explicit
  `null` values for schema/fixture reviewed reasons, trust and revocation
  material lists, booleans, numeric caps, business-service arrays, and amount
  minor-unit arrays now fail before digest-bound XSD summaries are emitted.
  Reviewed XSD gap reasons and profile-catalog identity strings also reject
  ASCII control characters at preflight time, and production readiness rejects
  digest-correct archived XSD summaries whose reviewed gap reasons are present
  but empty or non-string, or whose schema-backed fixtures still carry a
  missing-schema reason. Rejected XSD manifest and archived summary path
  validation errors report label-only failures without echoing raw path values
  that may contain secret-looking segments. Checked-in XSD source provenance paths now reject
  embedded whitespace and semicolon path parameters during preflight, and
  archived profile-catalog paths get the same readiness recheck when production
  readiness consumes archived XSD summaries.
- Added `scripts/iso_operator_evidence_verify.py`, an offline production
  evidence gate for archived canary and trust-bundle summaries. It caps each
  input JSON file at 4 MiB before parsing, recomputes summary digests, requires
  exact expected provider/environment CLI context,
  requires successful rail/notary/verify stages by default,
  requires the verify stage to carry digest-bound receipt-verifier JSON with
  rail and notary receipt kinds plus per-receipt digests, caps direct
  receipt-verifier stdout/stderr at 4 MiB before JSON parsing, bounds direct
  receipt-verifier runtime with positive finite
  `--receipt-verifier-timeout-secs`, rejects plan-only
  or dry-run canaries, plaintext-HTTP overrides, default-profile fallbacks,
  legacy `colr.007` local overrides, unredacted bearer-token paths,
  non-canonical compact receipt paths with control characters, surrounding
  whitespace, dot or parent traversal segments, or non-`*.receipt.json` leaves,
  secret-looking output, non-canonical canary runbook `config_path` values with
  control characters, surrounding whitespace, traversal segments, or non-JSON
  leaves, control-bearing or whitespace-padded provider, stage, receipt-kind,
  or trust-profile identity strings, unknown raw canary, receipt-verifier, or
  trust-bundle summary fields,
  smuggled trust-source URLs including repeated path separators, missing/malformed/future
	  trust-source retrieval timestamps, missing/malformed/future or padded
	  trust-summary `verified_at` timestamps, smuggled child command endpoint URLs
	  including overlong URLs, invalid or overlong hosts, repeated path separators,
	  localhost/local-private IP literals, known local/private rebinding hostnames,
	  legacy IPv4 numeric notation, IPv6 transition addresses embedding non-global
	  IPv4 addresses, and percent-escape smuggling, local-only
  child command flags in either `--flag` or `--flag=value` form, including the
  notary adapter's `--allow-missing-record-sources` diagnostic override,
  unsupported child command flags outside the expected rail/notary/receipt
  verifier CLI surfaces, duplicate singleton child command flags, boolean
  child command flags spelled with `=value`, non-positive or non-finite numeric
  child command flag values, non-canonical child command path values including
  leading-dash segments, control
  characters or surrounding whitespace in executed or plan-only child command arrays,
  and child command arrays that omit required stage inputs,
  non-canonical rail/notary `receipt_dir` values with control characters,
  surrounding or embedded whitespace, leading-dash path segments, semicolon path
  parameters, empty segments, raw backslashes, or traversal segments,
  rail/notary `receipt_dir`
  values that do not match the child command's single `--receipt-dir`,
  verify-stage commands that omit the rail/notary receipt directories generated by non-dry-run stages,
  synthetic trust DER, record-only trust policy, and placeholder trust-source
  provenance such as `placeholder`, `replace-before-production`, or
  `example.invalid` URLs before an archive is treated as production evidence.
  For archived trust-bundle summaries, it preserves the
  bundle-level `bundle_sha256`, rejects duplicate bundle digests independently
  from duplicate profile IDs, requires emitted profile overrides to match the
  top-level profile id, rail, and signature policy, rechecks override pin/OID
  and CRL/OCSP material counts against the verifier's material summary, binds
  CRL/OCSP override DER back to recorded summary digests and byte lengths,
  rejects malformed policy OIDs or non-canonical, oversized, or non-SEQUENCE
  CRL/OCSP base64, and fails closed on same-count trusted/revoked pin overlap
  attacks.
- Added `scripts/iso_production_readiness.py`, an offline release-readiness
  rollup for digest-bound XSD and operator evidence summaries. It requires
  XSD/evidence summary JSON inputs to stay within a 4 MiB pre-parse cap,
  exact provider/environment CLI context, strict schema-backed/fixture-backed
  XSD proof with canonical fixture schema references and reviewed-gap reason
  strings, production evidence policies,
  per-canary explicit-policy proof, full rail/notary/verify canary evidence,
  digest-bound direct receipt-archive
  verification with per-receipt digests, rail/notary receipt kinds, no legacy
  `colr.007` local overrides, and `require-verified` trust profiles with at
  least one public-key or X.509 trust pin plus required CRL/OCSP revocation
  checks and material before reporting the ISO corridor ready.
- Added checked-in operator canary runbook templates under
  `fixtures/iso20022/operator_canary/` for Swift CBPR+, Fedwire Funds, SEPA SCT
  Inst, and securities CSD profile families.
- Added `scripts/iso_trust_bundle_verify.py`, an offline XMLDSig/XAdES trust
  bundle preflight for operator rail PKI material. It caps bundle JSON at
  64 MiB before parsing, verifies canonical pins,
  digest-bound base64 DER envelopes with a pre-decode 1 MiB DER-size cap and
  lightweight semantic shape checks for X.509 certificates, X.509 CRLs, and
  OCSPResponse wrappers, duplicate and
  contradictory trust/revocation material, required CRL/OCSP material, HTTPS
  provenance without credentials, params, query strings, fragments, malformed
  bracket syntax, control characters, local/private IP literals, known
  local/private rebinding hostnames, legacy IPv4 numeric notation, or IPv6
  transition addresses embedding non-global IPv4 addresses, requires
  provenance URL and timezone-aware non-future retrieval timestamp fields before
  emitting a trust summary, rejects repeated input paths, copied bundles, and
  duplicate profile IDs, emits and de-duplicates bundle-level `bundle_sha256`
  values, enforces unique DER labels per material class, rejects
  present `null` or non-string DER-object `sha256` values before digest matching
  or profile override emission, omits absent DER labels from trust summaries,
  and rejects secret-looking fields before emitting Torii
  profile trust overrides.
- Added checked-in trust-bundle templates under
  `fixtures/iso20022/trust_bundles/` for Swift CBPR+, Fedwire Funds, SEPA SCT
  Inst, and securities CSD profile families.
- Made JS `submitIsoMessage` require an explicit `creationDateTime`; the helper
  no longer injects wall-clock time.
- Added `profile` support to JS ISO submissions and extended response
  normalization for profile/status-history fields.
- Added explicit `--iso-settlement-date YYYY-MM-DD` to CLI ISO settlement
  previews so generated `sese.023`/`sese.025` XML can be deterministic.

## Operator Canary Runbook

Use `scripts/iso_operator_canary.py` for provider or environment canaries that
need both live-rail delivery evidence and external audit-spool publication
evidence:

```bash
python3 scripts/iso_operator_canary.py \
  --config fixtures/iso20022/operator_canary/swift_cbpr_plus.preprod.example.json \
  --plan-only \
  --require-explicit-policy

python3 scripts/iso_operator_canary.py \
  --config runbooks/iso/local-provider-canary.json \
  --require-explicit-policy \
  --summary-out run/iso/local-provider-canary.summary.json
```

The JSON runbook is intentionally strict. It must label `provider` and
`environment`, configure at least one of `rail` or `notary`, and enables receipt
  verification by default. Relative paths are resolved from the runbook location
  with their parent directories canonicalized and their final path leaf
  preserved for child script boundary checks; they must stay under that
  directory. Optional path and numeric limit fields are optional only when
  absent; a present `null` value is malformed. Use absolute paths for explicitly
  external operator directories.
Endpoint URLs must not contain embedded credentials,
params, query strings, or fragments. Bearer-token files are runtime-only inputs
passed through to child scripts; the runner never reads token contents and
redacts token-file arguments in the summary. Production canaries should use
`--require-explicit-policy`, which requires every runbook policy boolean to be
present and records that proof in the summary for the evidence gate; regression
coverage removes each rail, notary, and verifier policy boolean in turn. Checked-in
templates for Swift CBPR+, Fedwire Funds, SEPA SCT Inst, and securities CSD
live-profile families live under
`fixtures/iso20022/operator_canary/`; copy them into an operator runbook area
before replacing placeholder endpoints, token-file paths, inboxes, and
`audit_export_dir` locations.

```json
{
  "provider": "example-bank",
  "environment": "preprod",
  "rail": {
    "inbox_dir": "inbox",
    "torii_base_url": "https://torii.example.internal",
    "receipt_dir": "receipts/rail"
  },
  "notary": {
    "export_dir": "audit-export",
    "endpoints": ["https://notary.example.internal/iso-anchor"],
    "receipt_dir": "receipts/notary"
  },
  "verify": {
    "allow_insecure_http": false,
    "require_source_files": true
  }
}
```

## Trust Bundle Preflight

Use `scripts/iso_trust_bundle_verify.py` before merging rail PKI trust material
into an ISO bridge profile:

```bash
python3 scripts/iso_trust_bundle_verify.py \
  --bundle fixtures/iso20022/trust_bundles/swift_cbpr_plus.preprod.example.json \
  --allow-synthetic-der \
  --summary-out run/iso/swift-cbpr-plus-trust-summary.json
```

The verifier is offline. It does not replace Torii's semantic X.509, CRL, and
OCSP checks; it catches operator-package mistakes earlier by requiring
canonical lowercase trust profile IDs, rail values from the supported ISO rail
catalog (`generic-iso20022`, `swift-cbpr-plus`, `fedwire-funds`,
`sepa-sct-inst`, and `securities-csd`), lowercase nonzero SHA-256 pins, matching
DER digests, no duplicate DER within a material class, unique DER labels within
each material class, no trust anchor that is also revoked, CRL/OCSP material when
the corresponding profile flags are explicitly enabled, explicit CRL/OCSP
revocation policy booleans, clean HTTPS source URLs by default, including the
2048-character URL cap and DNS 253-character host limit,
legacy IPv4 numeric notation rejection, IPv6 transition embedded-IPv4 rejection,
timezone-aware non-future
`source.retrieved_at` values, required clean `source.authority`/`source.version`
provenance, non-placeholder production source metadata and
non-`example.invalid` source URLs in production evidence/readiness gates, and
no runtime secret fields. DER-object `sha256` values are optional only when the key
is absent; present `null` or other non-string values are malformed for trust
anchors, revoked certificates, CRLs, and OCSP responses. DER labels are emitted
only when present and non-empty; archived summaries that explicitly carry
`label: null` fail production-evidence verification. Production bundles
also need DER values that look like the expected material class; the checked-in
templates use synthetic DER envelopes only so CI can validate schema and
emitted-profile wiring, and require `--allow-synthetic-der`.
Synthetic-template validation is summary-only:
`--allow-synthetic-der` cannot be combined with `--emit-profile-json`. Replace
templates with the current rail PKI package before production use and omit that
flag before emitting profile overrides; profile emission also refuses
`--allow-record-only`, `--allow-insecure-source-url`, placeholder
authority/version strings, `example.invalid` source URLs, missing source
freshness budgets, and stale source retrieval timestamps:

```bash
python3 scripts/iso_trust_bundle_verify.py \
  --bundle run/iso/provider-swift-cbpr-plus-trust-bundle.json \
  --summary-out run/iso/swift-cbpr-plus-trust-summary.json \
  --max-source-age-days 7 \
  --emit-profile-json run/iso/swift-cbpr-plus-profile-overrides.json
```

Production evidence requires the trust summary to prove both
`profile_json_emittable=true` and `profile_json_emitted=true`; a summary that
could emit profile overrides but did not write them is local-audit evidence only.
Summaries that claim profile JSON was emitted while the archived source policy is
not emittable are rejected even under local-audit allowances.
When profile overrides are emitted, the trust summary is written only after the
profile JSON file write succeeds and carries `profile_json_sha256` for the exact
emitted body. The evidence gate recomputes that digest from the archived
`profile_overrides` before accepting the trust summary.

## Production Evidence Gate

After a live canary and trust preflight complete, archive only evidence that
passes `scripts/iso_operator_evidence_verify.py` with production defaults:

```bash
python3 scripts/iso_operator_evidence_verify.py \
  --canary-summary run/iso/local-provider-canary.summary.json \
  --trust-summary run/iso/swift-cbpr-plus-trust-summary.json \
  --receipt-dir run/iso/rail-receipts \
  --receipt-dir run/iso/notary-receipts \
  --provider example-bank \
  --environment preprod \
  --max-canary-age-days 1 \
  --max-trust-age-days 7 \
  --max-trust-source-age-days 7 \
  --summary-out run/iso/local-provider-evidence.summary.json
```

The gate is offline and fails closed. The command must name the expected
`--provider` and `--environment`, which are rechecked against canary and trust
evidence before an archive can be accepted and recorded in the digest-bound
evidence policy. It must also name explicit freshness budgets for canary
`finished_at`, trust-summary `verified_at`, and trust-source `retrieved_at`
timestamps; digest-correct stale archive inputs are rejected before archival.
Accepted trust profiles preserve their compact `bundle_sha256`, optional source
`authority`/`version`, source URL, `retrieved_at` timestamp, the trust verifier's
`max_source_age_days` emission budget, revoked-certificate pin count, required
certificate-policy OID count, and compact trust-anchor/revoked/CRL/OCSP DER
proof digests and byte lengths for the final readiness rollup. The compact trust
summary also preserves and rechecks that `profile_json_emittable` matches the
archived trust source policy, and preserves
`profile_json_sha256` so release evidence names the exact profile override body
produced by the trust preflight; the evidence gate recomputes it from the
archived profile override objects before archival.
Repeated canary/trust summary paths or copied summaries with the same
`summary_sha256` are malformed input. By default it requires
digest-bound, successful canary summaries with rail, notary, and
receipt-verify stages in canary-runner order;
requires the canary summary to prove it was generated with
`--require-explicit-policy`;
requires the receipt-verify stage output to be digest-bound and contain
positive receipt counts, duplicate-free rail/notary receipt-kind lists, and
per-receipt `receipt_sha256` entries with unique receipt paths and unique
receipt digests, explicit `ok=true` plus 2xx `status_code` metadata on every
receipt entry, kind-specific notary anchor/index/count metadata or rail
message/profile/payload metadata, and explicit boolean receipt policy fields
rather than omitted defaults;
rejects timed-out stages and truncated child-process stdout or stderr previews
from every executed canary stage before trusting the archived stage output;
requires timeout-bounded direct receipt archive verification from `--receipt` or
`--receipt-dir` before emitting an evidence summary, and requires that direct archive's
`receipt_sha256` entries cover every canary receipt-summary digest with the same
receipt kind, successful status metadata, and kind-specific receipt metadata;
rejects plan-only, dry-run, insecure-HTTP,
default-profile, failed-receipt, legacy `colr.007`, and missing-source-file
evidence; requires trust summaries produced without synthetic DER, record-only
policy, insecure provenance overrides, or malformed/future trust-source
retrieval timestamps, with trust-summary policy booleans present explicitly and
with profile override JSON actually emitted;
requires archived trust profile overrides to carry explicit CRL/OCSP revocation
policy booleans, unique canonical lowercase profile IDs and bundle digests
across archived trust summaries, known rail IDs, matching profile/rail/policy
override identities, canonical
OIDs, bounded canonical base64 DER SEQUENCE blobs, override material counts, and
CRL/OCSP DER digests and byte lengths that agree with the trust-bundle verifier
summary; and scans
archived commands/output for obvious secret leakage.
Plan-only diagnostic archives must still record each planned stage's `dry_run`
boolean. Bearer-token file arguments must be redacted whether represented as
`--bearer-token-file <path>` or `--bearer-token-file=<path>`. The `--allow-*`
flags, including `--allow-legacy-colr007`,
`--allow-missing-record-sources`, `--allow-canary-stage-receipts-only`, and
`--allow-profile-json-not-emitted`, are for local test audits only and should not
be present in production evidence archives.

## Production Readiness Rollup

Use `scripts/iso_production_readiness.py` as the final offline release gate
after the XSD manifest preflight and operator evidence gate emit their
digest-bound summaries:

```bash
python3 scripts/iso_production_readiness.py \
  --xsd-summary run/iso/xsd-fixture-summary.json \
  --evidence-summary run/iso/local-provider-evidence.summary.json \
  --provider example-bank \
  --environment preprod \
  --max-xsd-age-days 30 \
  --max-evidence-age-days 7 \
  --max-canary-age-days 1 \
  --max-trust-age-days 7 \
  --max-trust-source-age-days 7 \
  --summary-out run/iso/production-readiness.summary.json
```

The rollup exits `0` only when every supplied summary proves production posture.
The release command must name the expected `--provider` and `--environment`,
which are rechecked against canary and trust evidence instead of being inferred
from archived summaries. It must also name explicit freshness budgets for XSD
summaries, aggregate evidence summaries, canary `finished_at` timestamps, and
trust-summary `verified_at` timestamps, plus trust-source `retrieved_at`
timestamps captured by the evidence archive policy. Digest-correct but stale
inputs become production blockers instead of silently passing release posture.
The rollup also rechecks the digest-bound provider/environment and freshness
policy recorded by the evidence gate, rejects archive freshness budgets that
are weaker than the final release budgets, requires and revalidates each compact
trust profile's canonical lowercase profile ID, known rail ID, source
`authority`/`version`, source URL, and `retrieved_at` timestamp, rejects stale or
smuggled trust source provenance including
raw-whitespace and empty/zero/leading-zero/malformed-port URL smuggling plus non-canonical host spelling,
invalid host labels, percent-host and percent-path smuggling including encoded
semicolon parameters and encoded URL delimiters, numeric-host
spoofing, repeated path separators, and path traversal, rejects omitted,
malformed, evidence-weaker, or release-weaker compact trust source freshness
budgets, rejects compact `profile_json_emittable` values that no longer match
the archived source provenance and freshness budget, rejects compact summaries
that report emitted profile JSON while not emittable, rejects compact
canary/trust summary paths,
canary config paths, and receipt paths with embedded whitespace, semicolon path
  parameters, leading dashes, leading-dash path segments, empty segments, raw
  backslashes, or dot/parent traversal, rejects repeated XSD/evidence
summary paths or copied summaries with the same `summary_sha256`, and rechecks
digest-bound XSD `schemas[]` and
`fixtures[]` arrays for count consistency, unique schema and fixture evidence
digests, cross-summary schema/fixture path and digest replay, canonical relative schema paths whose filenames match
`message_def_id`, fixture paths that remain relative forward-slash XML paths
without leading dashes, empty, dot, or non-leading parent segments,
schema-backed/missing-schema consistency, and schema-backed fixture XML
schema-validation proof. It also rechecks profile-catalog source digests,
version coverage counts, canonical profile ids, ISO family message types,
allowed directions, message-definition family binding, and missing-version
entries, plus skipped family-version shape and profile catalog count
consistency, rejects unknown XSD summary fields across strict flags,
schema/fixture/gap/profile-catalog entries,
and the XSD preflight rejects unknown keys in source profile/message catalog
entries plus present-null optional manifest/profile fields before such fields
can reach the summary,
recomputes `profile_catalog.missing_schema_versions`, and cross-checks
schema-only flags and reviewed reasons against the schema/fixture relationship.
Reviewed `missing_schema_fixtures` and `schema_only_entries` must also match the
recomputed exact path, `message_def_id`, and reason tuples.
It blocks summaries that do not prove `--require-profile-schema-backed-versions`. The
rollup rechecks the direct receipt archive verification emitted by the evidence
gate from `--receipt` or `--receipt-dir`, not only the canary stage's captured
verifier stdout, and blocks digest-correct evidence summaries whose direct
archive receipt digests no longer cover the canary receipt-summary digests or
whose direct archive carries receipt digests that no canary references, or whose
canary entries relabel the archived receipt kind or drift from the archived
successful status or kind-specific receipt metadata. Distinct canary summaries
must not reuse compact receipt paths or receipt digests. That
archive verification summary must carry its own
`summary_sha256`, production policy flags, and a `receipts[]` list binding each
unique canonical `*.receipt.json` path to its unique `receipt_sha256` and one
of the supported rail/notary receipt evidence kinds while preserving
`ok=true`, 2xx `status_code` success metadata, and the corresponding
kind-specific notary or rail compact metadata. XSD
strict-mode flags, evidence-level production policy flags, and nested
receipt-summary policy flags must all be present as booleans; evidence `ok`,
canary `plan_only`, and
per-canary `require_explicit_policy` status fields are also required booleans.
Trust-summary policy flags are enforced by the evidence gate before the rollup
accepts an archive, and the rollup independently rechecks compact trust-profile
`bundle_sha256`, CRL/OCSP revocation booleans, material counts, revoked and
certificate-policy counts, compact DER proof shape and count binding, and
`verified_bundles` profile-count binding. Compact trust summaries must also
retain `profile_json_emitted=true`, `profile_json_emittable=true`, and a
lowercase `profile_json_sha256`; false values, missing or malformed profile JSON
digests, duplicated compact trust-profile IDs, or duplicated bundle digests are
production blockers, and `profile_json_emittable` is recomputed from the compact
source evidence before the rollup is accepted.
The rollup requires at least one XSD summary and at least
one evidence summary, and compact canary and trust summary entries must also
retain control-free, trim-free, leading-dash-free, traversal-free source paths and canonical lowercase
`summary_sha256` pointers; nested canary, trust, archive-receipt, and
trust-profile material must not be replayed across distinct evidence summaries.
Compact canary entries also retain the validated
runbook `config_path`, which must remain control-free, trim-free, leading-dash-free, traversal-free, and point
to a `.json` file. Compact provider/environment, stage, receipt-kind,
and trust-profile identity strings also reject control characters and
surrounding whitespace instead of being trimmed. Unknown compact fields are malformed across evidence policy, canary,
stage-window, receipt-summary, receipt-entry, trust-summary, and trust-profile
objects. Compact trust summaries must retain timezone-aware, non-future
`verified_at` timestamps for the original trust-bundle preflight. Omitted inputs,
flags, or digest pointers are malformed, as are missing, timezone-less, or future
XSD/evidence/trust `verified_at` timestamps and malformed or reversed canary
`started_at`/`finished_at` windows. The canary runner also records per-stage
windows, and the evidence/readiness gates reject compact `stage_windows` when
they are missing, malformed, outside the canary window, or name-mismatched; they
also reject reordered or overlapping stage windows and duplicate or unsupported
compact stage names, including compact stage-name sequences that do not follow
rail/notary/verify order. These conditions are malformed input, not implicit
production defaults. It exits `1` with a
digest-bound blocker report when summaries are valid but not production-ready,
and exits `2`
for malformed or digest-tampered inputs, including nested receipt-summary
tampering. Evidence summaries or nested receipt summaries that were produced with
`allow_legacy_colr007=true`, or canary summaries that do not prove
`--require-explicit-policy`, are production blockers. Trust profiles that disable
CRL/OCSP revocation checks or carry zero required revocation material are also
production blockers. Compact trust profiles whose source authority/version still
contains template markers such as `placeholder` or `replace-before-production`,
or whose source URL still points at `example.invalid`, are production blockers
even when the DER material itself is real. Compact trust-source URLs also fail
closed on overlong URLs and DNS hosts instead of relying on downstream URL
consumers.
`--allow-reviewed-xsd-gaps` and `--allow-canary-stage-receipts-only` exist for
local diagnostic audits of the current checked-in fixture corpus; production
release evidence should omit them and must make the strict XSD, profile-catalog,
and receipt-archive checks pass.

## Gap Register

| Area | Current state | Target |
| --- | --- | --- |
| Rail connectivity | Local bridge endpoints plus `scripts/iso_rail_gateway_adapter.py`, an operator-side file-drop adapter that verifies sidecar-pinned message type/profile/payload digest and rejects duplicate payload digests or duplicate rail message ids before submitting to Torii and writing receipts outside consensus-critical code; `colr.012` is the production collateral-substitution family and legacy `colr.007` requires an explicit local override that receipt/evidence/readiness gates reject for production; `scripts/iso_operator_receipt_verify.py` gates the resulting receipts, `scripts/iso_operator_canary.py` ties the adapter plus verifier into a reproducible provider runbook, `scripts/iso_operator_evidence_verify.py` rejects non-production archived summaries, `scripts/iso_production_readiness.py` aggregates accepted summaries into one release gate, and checked-in Swift/Fedwire/SEPA/CSD templates plan successfully without network access | Run provider-specific live gateway canaries for selected SWIFT/Fedwire/SEPA/CSD operators and archive evidence summaries that pass the production-readiness gate |
| XMLDSig/XAdES | Supported P-256/SHA-256 enveloped subset is verified against profile public-key, leaf-certificate, and linked certificate-chain pins with non-CA XMLDSig leaf certificates carrying critical `keyUsage`/`digitalSignature`, deterministic child issuer distinguished-name binding to parent subject distinguished names, bounded duplicate-free `X509Data` certificate chains, certificate-chain ECDSA-with-SHA256/id-ecPublicKey-secp256r1 enforcement, critical issuer CA `basicConstraints` and `keyUsage`/`keyCertSign`, issuer path-length constraint enforcement, rejection for unknown, malformed, or unsupported parsed critical X.509 extensions, extension/validity checks against verified signed XAdES `SigningTime` or BAH `CreDt`, explicit certificate revocation pins, low-S fixed-width `r || s` or low-S DER ECDSA signature values, one empty or unique same-document `#id` payload Reference URI that strictly encloses the verified signature carrier with an enveloped-signature transform first, at most one final supported C14N transform, one optional XAdES `SignedProperties` Reference with a local `#id`, `QualifyingProperties` target bound to the enclosing `Signature` `Id`, exactly one supported bare `Signature` or direct-child `Sgntr`/`Signature` carrier, ordered direct `Signature`/`SignedInfo` child parsing, prefixed XMLDSig structure bound to the XMLDSig namespace, prefixed XAdES structure bound to the ETSI XAdES v1.3.2 namespace, exact QName opening/closing tag matching in supported XML spans, malformed structural QName rejection, direct `Signature/Object/QualifyingProperties/SignedProperties/SignedSignatureProperties` XAdES parsing, certificate-backed `SigningCertificateV2` ordered duplicate-free chain-prefix digest binding with direct `Cert`/`CertDigest` children only, no unreferenced, wrapped, or duplicate `SignedProperties` elements, parameter-free XMLDSig method/transform elements with exact-one critical methods, exact-name method/digest/Reference policy attribute lookup, exact-one attribute-free `Transforms` wrappers, implemented ordinary attributes only, and ordered direct `Reference` children, singleton required base64 values, unambiguous public-key-or-certificate key material scoped to exactly one structured `KeyInfo`, inherited namespace context for referenced roots, a fail-closed supported canonical XML subset for empty-element expansion, simple attribute normalization, namespace-aware attribute sorting, implicit `xml:` namespace attributes, legal `xmlns:xml` declaration omission, XML-character-reference decoding, no-comments C14N comment omission, and C14N-mode-specific root namespace declarations inherited from the enclosing `Signature`; `scripts/iso_trust_bundle_verify.py` preflights operator trust bundles, `scripts/iso_operator_evidence_verify.py` rejects synthetic/record-only trust summaries for production archives, `scripts/iso_production_readiness.py` rechecks production trust posture in the release rollup, and checked-in Swift/Fedwire/SEPA/CSD templates validate schema and emitted trust overrides | Replace synthetic trust-bundle templates with official profile-specific trust-anchor packages, add complete official canonical XML fixture coverage, add official CRL/OCSP or rail revocation-feed fixtures, and archive evidence summaries that pass the production-readiness gate |
| Follow-up messages | Inbound `pacs.002`, `pacs.004`, `camt.056`, `sese.023`, `sese.024`, `sese.025`, and `colr.012` lifecycle endpoints record durable messages, reject replay evidence, and update known referenced originals only; checked-in payment, securities, and collateral XML fixtures now cover `pacs.002`, `pacs.004`, `camt.056`, `sese.023`, `sese.024`, `sese.025`, and `colr.012` profile/lifecycle handling; the offline MDR/XSD fixture matrix now includes standalone fixtures for every checked-in payment XSD, including `pacs.008.001.08`, `pacs.009.001.08`, `pacs.002.001.10`, `pacs.004.001.10`, `camt.056.001.08`, and `camt.056.001.09`; `scripts/iso_xsd_fixture_verify.py` prevents silent XSD/XML fixture namespace and payload-root drift while recording reviewed missing-schema gaps and profile-advertised message-version gaps | Add remaining official MDR/XSD lifecycle fixtures per profile, make the strict schema-backed XSD/profile preflight pass, and add live-rail gateway adapter coverage |
| Return/cancel lifecycle | Durable outbox helpers exist for `pacs.004`, `camt.029`, `sese.024`, and `sese.025`; known-original return and cancellation transitions have focused Torii coverage plus checked-in `pacs.004` and `camt.056` XML fixtures; full `pacs.004.001.10`, `camt.056.001.08`, and `camt.056.001.09` XSD fixtures now pin live-profile return/cancellation admission where the default rail profiles allow those versions | Add remaining official rail/profile return and cancellation fixture packs |
| Securities crosswalks | Reference snapshots load locally and live securities profile admission validates instrument, active venue MIC, delivering/receiving BIC lookups, configured CSD venue domain, delivering/receiving settlement-account mappings, and securities cash-leg asset mapping before durable lifecycle recording | Keep operator snapshots current and add live-rail adapter coverage around production CSD/account/cash-leg sources |
| Profile catalog | Static defaults plus config overrides; the XSD preflight can now parse the embedded default catalog and report schema-backed coverage for concrete profile-advertised versions | Add fixture coverage against official MDR/XSD releases per profile until `--require-profile-schema-backed-versions` passes |
| Persistence | Digest-bound local JSON state files plus deterministic local audit index; tampered records are rejected on reload, excluded from regenerated indexes, the current manifest is exposed through `GET /v1/iso20022/audit/messages`, configured age/count retention compacts records while regenerating the manifest, `audit_export_dir` mirrors digest-bound manifest/notary preimages to an operator-managed external spool, `scripts/iso_audit_notary_adapter.py` verifies and publishes those preimages to configured HTTPS archival/notary endpoints with local receipts, `scripts/iso_operator_receipt_verify.py` gates those receipts, `scripts/iso_operator_canary.py` records one rail/notary/verify summary, `scripts/iso_operator_evidence_verify.py` rejects non-production summaries before archival, and `scripts/iso_production_readiness.py` aggregates XSD/evidence summaries into the final release report | Run provider-specific production canaries against the selected archival/notary vendors and archive evidence summaries that pass the production-readiness gate |

## Public Interface Notes

- Submission/status endpoints:
  - `POST /v1/iso20022/pacs008`
  - `POST /v1/iso20022/pacs009`
  - `POST /v1/iso20022/pacs002`
  - `POST /v1/iso20022/pacs004`
  - `POST /v1/iso20022/camt056`
  - `POST /v1/iso20022/sese023`
  - `POST /v1/iso20022/sese024`
  - `POST /v1/iso20022/sese025`
  - `POST /v1/iso20022/colr012`
  - `GET /v1/iso20022/status/{msg_id}`
- New/readable endpoints:
  - `GET /v1/iso20022/messages/{msg_id}`
  - `GET /v1/iso20022/audit/messages`
  - `GET /v1/iso20022/messages/{msg_id}/pacs002`
- Submission responses and status records now include `profile_id`,
  `message_type`, `business_service`, `business_message_id`, `uetr`,
  `payload_hash`, `reference_snapshot_id`, `embedded_signature_detected`, and
  `status_history`, plus lifecycle-family context such as securities settlement
  fields and collateral substitution fields when present.

[^iso_catalogue]: ISO 20022 Catalogue of messages. https://www.iso20022.org/catalogue-messages
[^swift_cbpr]: Swift, "ISO 20022: A new era for global payments", 25 November 2025. https://www.swift.com/news-events/news/iso-20022-new-era-global-payments
[^fedwire_iso]: Federal Reserve Financial Services, "Fedwire Funds Service Completes ISO 20022 Migration", 16 July 2025. https://www.frbservices.org/news/fed360/issues/071625/wires-iso-20022-implementation-complete-fedwire-funds-service
[^cpmi_harmonisation]: BIS CPMI, "Harmonised ISO 20022 data requirements for enhancing cross-border payments - updated report", 26 February 2026. https://www.bis.org/cpmi/publ/d230.htm
