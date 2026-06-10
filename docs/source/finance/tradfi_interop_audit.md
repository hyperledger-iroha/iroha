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
  rejected along with reserved placeholder hosts, checked-in template hosts
  under `operator-canary.bank`, invalid DNS labels, percent-escaped hosts, numeric-host
  spoofing, traversal, encoded-separator, encoded-semicolon, encoded URL
  delimiters, encoded-percent, percent-encoded
  control/space bytes, malformed percent escapes, backslash, and embedded
  semicolon path smuggling, duplicate publication endpoints are rejected before
  network delivery without echoing the endpoint URL, remote redirects are not
  followed and are archived as failed receipts, bearer-token files must be
  regular non-symlink inputs
  capped at 8 KiB before decoding to exact UTF-8 with no surrounding
  whitespace, embedded whitespace, or control characters, with token-file
  read/decode/size failures reported by input label instead of runtime path, and
  the export directory plus export inputs (`latest.notary.json`, the
  digest-addressed anchor peer, `messages.index.json`, and
  clean `store_dir/messages` record sources) must be non-symlink regular
  directories/files. The `--export-dir` and `--bearer-token-file` CLI paths
  reject raw control characters, whitespace, leading-dash segments,
  backslashes, semicolon parameters, secret-looking key/value material, empty
  segments, and dot/parent traversal before argparse `Path` normalization,
  embedded anchor `store_dir` paths reject the same secret-looking key/value
  material before record-source verification,
  and `--endpoint` rejects missing, empty, or flag-looking URL values before
  argparse parsing.
  Malformed `--timeout-secs` and `--response-limit-bytes` CLI values fail before
  argparse can echo raw operator input; parsed `--timeout-secs` values must be
  positive and finite, and `--response-limit-bytes` must be a positive integer.
  Anchor/index JSON input is capped at 64 MiB, persisted record-source JSON
  is capped at 1 MiB, and diagnostic `--allow-missing-record-sources` is
  rejected unless at least one validated anchor actually lacks its local record
  sources before publication. The adapter writes each bounded receipt without
  persisting bearer-token material, rejecting secret-looking or
  control-bearing successful remote response bodies before receipt persistence,
  and redacting failed remote response previews or transport errors before
  persistence. Rejected
  endpoint URL validation errors, duplicate endpoint errors, and oversized
  response errors report the structural failure by field label without echoing
  raw URL strings that may contain query secrets or private topology.
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
  `pacs.004.001.09`, `pacs.004.001.10`, `camt.056.001.08`, and
  `camt.056.001.09` to the offline MDR-derived schema set. The live-profile
  fixture matrix now admits BAH status, return, and cancellation reports,
  including profile-advertised `pacs.004.001.09` and `camt.056.001.09`
  variants, under the configured version/business-service controls for the rails
  that allow those exact versions.
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
  finite `--xmllint-timeout-secs` capped at 300 seconds during optional schema validation, and
  accepts only empty successful output or the normal `<fixture> validates`
  success line so warning-bearing successful validator output fails closed,
  enforces canonical lowercase ISO message definition ids and path
  containment for schema/fixture entries, rejects manifest schema paths,
  fixture paths, and fixture schema references longer than 2048 characters or
  with non-ASCII characters, URI/drive prefixes, malformed percent escapes,
  percent-encoded control/space, dot/separator, semicolon, URL delimiter, or
  percent bytes, backslashes, embedded whitespace, leading-dash path segments,
  semicolon path parameters, empty segments, dot segments, or forbidden parent
  segments, rejects copied XML fixtures with duplicate
  fixture SHA-256 values, optionally validates schema-backed XML fixtures
  against their checked-in XSDs with `xmllint --nonet`, optionally
  parses the embedded default rail profile catalog and records which concrete
  advertised message versions are schema-backed, and the strict
  `--require-profile-schema-backed-versions` gate uses that default catalog when
  no `--profile-catalog` override is supplied, emits versioned digest-bound summaries,
  requires manifest `version` to be the exact integer `1` rather than a JSON
  boolean, recursively rejects secret-looking profile-catalog strings before
  rail, signature-policy, reference-dataset, address-mode, or version diagnostics
  can echo catalog-provided values, requires profile-catalog enum and list values
  such as rails, embedded signature policies, reference datasets,
  structured-address modes, and business services to be printable ASCII, applies
  the same profile-catalog scanner and overlong-value caps to identifier-style
  strings such as profile ids and business-service entries, rejects
  secret-looking or non-ASCII schema and fixture `payload_root`
  values before namespace/root mismatch diagnostics can echo manifest-provided
  payload names, and rejects secret-looking or non-ASCII checked-in XSD
  `targetNamespace` attributes before schema namespace mismatch diagnostics can
  echo schema-provided attribute values, with the same label-only treatment for
  XSD/fixture payload identifiers, XML fixture namespace/name identifiers, and
  schema-root attribute names, caps those XSD/XML schema and fixture identifiers
  before schema/root mismatch diagnostics can print overlong ASCII spellings,
  scans XML fixture contents before optional
  `xmllint` validation, and redacts secret-looking, control-bearing, or
  non-ASCII validator output before composing XSD preflight diagnostics,
  records manifest, schema, fixture, profile source-file, and embedded catalog
  JSON SHA-256 provenance, requires exactly one active Rust
  `DEFAULT_PROFILES_JSON` raw-string declaration and ignores spoofed matches in
  comments or unrelated strings, requires each checked-in XSD to carry canonical
  repository, commit, source path, SPDX license, and source SHA-256 provenance
  that matches the checked-in bytes, caps source repository URLs and source
  provenance paths at 2048 characters, rejects placeholder repository owners or names such as
  `example`, `dummy`, `fake`, `sample`, or `template`, rejects source
  provenance paths with non-ASCII characters, embedded whitespace, leading-dash
  path segments, semicolon path parameters, or identifier-style secret-looking material, and
  rejects omitted `source` separately from explicit null source objects in both
  direct preflight and archived readiness replay,
  requires the `blocked_schema_sources` review list to be explicitly recorded
  even when empty,
  rejects XSDs that contain
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
  redistributable profile-advertised payment, securities, and collateral XSD
  packages are checked in; an inspected `pacs.008.001.10` candidate still
  carries restricted redistribution terms and is intentionally not imported.
  The fixture manifest now records blocked public candidate-source evidence for
  `pacs.002.001.12`, `pacs.008.001.10`, and `pacs.009.001.10`, including the
  audited GitHub source path, commit, candidate SHA-256, and restriction-marker
  classes without checking in the restricted XSD bytes. Blocked candidates must
  now cite at least one explicit redistribution or public-distribution
  restriction marker; a generic copyright marker alone is not accepted as
  production blocker evidence.
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
	  non-global IPv4 addresses, reserved placeholder hosts such as `.example`,
	  `example.com`, `example.net`, `example.org`, or `example.invalid`, and
	  checked-in template hosts under `operator-canary.bank`,
	  empty/zero/leading-zero/malformed/default ports,
  non-canonical hosts, invalid host labels
  or numeric-host/legacy-IPv4 spoofing, percent-encoded control/space bytes, malformed
  percent escapes, and smuggled URL paths including encoded semicolon parameters,
  encoded URL delimiters, and repeated separators, keeps
  explicit `--message` paths inside the declared inbox, requires an explicit
  profile by default, rejects duplicate payload digests or duplicate
  `rail_message_id` values within one gateway run before network delivery,
  accepts only the Torii ISO message families in the adapter endpoint map
  (`pacs.008`, `pacs.009`, `pacs.002`, `pacs.004`, `camt.056`, `sese.023`,
  `sese.024`, `sese.025`, `colr.007`, and `colr.012`),
  rejects sidecar `profile` and `rail_message_id` values
  that are explicitly `null` or carry surrounding whitespace, embedded
  whitespace, or control characters, rejects non-canonical sidecar profile IDs,
  rejects sidecar `rail_message_id` values that are longer than 128 characters
  or are not canonical ASCII rail-message identifiers, rejects secret-looking
  identifier-style profile or rail-message-id markers before receipt emission
  or network delivery, rejects secret-looking `message_type` markers and
  malformed secret-looking `payload_sha256` values before unsupported-type or
  digest-mismatch diagnostics can echo those values, rejects unknown sidecar
  fields, rejects secret-looking material in known sidecar fields before
  unsupported-value diagnostics can echo message types, profiles, payload
  digests, or rail-message IDs, bounds sidecar JSON before parsing,
	  rejects legacy `colr.007`
	  collateral drops unless `--allow-legacy-colr007` is set for local diagnostics,
  rejects unused `--allow-insecure-http`, `--allow-default-profile`, and
  `--allow-legacy-colr007` flags unless the validated Torii URL or sidecars
  actually require the corresponding local diagnostic policy,
	  requires bearer-token files to be regular non-symlink inputs capped at 8 KiB
  before decoding to exact UTF-8 with no surrounding whitespace, embedded
  whitespace, or control characters, with token-file read/decode/size failures
  reported by input label instead of runtime path, rejects raw `--inbox-dir`,
  explicit `--message`, and `--bearer-token-file` CLI path smuggling before argparse `Path`
  normalization, rejects secret-looking key/value material in local paths before
  receipt output, rejects missing, empty, or flag-looking `--torii-base-url`
  values before argparse parsing, rejects malformed numeric CLI values before
  argparse can echo raw operator input, requires positive finite
  `--timeout-secs`, and requires positive integer `--max-payload-bytes` and `--response-limit-bytes`,
  does not follow remote redirects, and writes bounded local receipts for
  successful and failed submissions
  without persisting token material, rejecting secret-looking or control-bearing
  successful remote response bodies before receipt persistence, and redacting
  failed remote response previews or transport errors before persistence.
  Duplicate gateway payload and rail-message-id diagnostics report only field
  indexes, not the repeated digest or identifier value. Rejected Torii URL
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
  explicitly enabled for local tests, rejects unused `--allow-failed`,
  `--allow-insecure-http`, `--allow-legacy-colr007`, and
  `--allow-default-profile` flags unless verified receipts actually carry failed
  status, an HTTP/local endpoint, legacy `colr.007`, or a missing rail profile,
  records version-2 compact `endpoint_requires_insecure_http` evidence per
  receipt so archived replay can bind insecure-HTTP policy without persisting raw
  endpoint URLs and rejects summaries that hide that endpoint evidence behind
  `allow_insecure_http=false`,
  rejects receipt endpoint URLs with
  credentials, params, query strings, fragments, malformed hosts, surrounding
  or embedded whitespace, control characters, empty/zero/leading-zero/malformed/default ports, or
  non-canonical host spelling, localhost/local-private IP literals, known
  local/private rebinding hostnames, IPv6 transition addresses embedding
  non-global IPv4 addresses, reserved placeholder hosts such as `.example`,
  `example.com`, `example.net`, `example.org`, or `example.invalid`, and
  checked-in template hosts under `operator-canary.bank`, invalid host labels,
  percent-escaped hosts, numeric-host/legacy-IPv4 spoofing, plus
  path traversal, encoded path separators, repeated path separators,
  encoded-semicolon parameters, encoded URL delimiters, encoded-percent path
  segments, or percent-encoded control/space bytes,
  detects leaked authorization/token material plus secret-looking allowed
  receipt string values, including malformed `receipt_kind`, before version or
  kind dispatch and without echoing those values, reports rejected endpoint
  URL structure and duplicate receipt digest failures without echoing raw URL
  strings or receipt paths that may contain query/path secrets, closes the raw
	  receipt schemas for each receipt kind plus notary
	  anchor/audit-index source schemas, including an explicit `records[]` array
	  and duplicate-free nested audit records, binds
  audit record filenames to `sha256(message_id).json`, binds each indexed
  `record_sha256` to the persisted `store_dir/messages` body when source files
  are required or locally available, rejects row/source metadata drift and
  persisted-state-derived `pacs002_code` or status-history timestamp drift,
  binds recorded endpoint digests to the recorded endpoint URLs, requires
  timezone-aware adapter timestamps that do not require trimming, enforces
  `ok`/`status_code` consistency, requires HTTP response body digests and
  failed-receipt error strings, validates bounded response metadata, can
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
  requires raw rail receipts and archived rail receipt summaries to record
  nullable `profile`/`rail_message_id` keys,
  plus receipt and source-sidecar rail
  `profile`/`rail_message_id` values when they carry surrounding whitespace or
  embedded whitespace or control characters, rejects source-sidecar explicit
  null optional metadata instead of treating it as omission, rejects non-canonical receipt or
  source-sidecar profile IDs, rejects overlong or non-canonical ASCII
  `rail_message_id` values, rejects archived receipt and source-sidecar
  profile or rail-message-id values that look like secret-bearing identifiers,
  rejects archived receipt `message_type` values that look like secret-bearing
  identifiers before receipt-summary emission, rejects malformed archived rail
  receipt `message_type` family IDs and Unicode digit confusables before
  allowlist checks, rejects archived
  rail receipt `message_type` values outside the same adapter endpoint map
  before receipt-summary emission, caps receipt JSON at 4 MiB, notary
  anchor/index JSON at 64 MiB, persisted notary record-source JSON at 1 MiB,
  rail source XML at 4 MiB, and source-sidecar JSON at 16 KiB before parsing or
  hashing, replays digest-addressed notary-anchor and
  `messages.index.json` checks while rejecting symlinked or non-regular notary
  anchor/index peers and rail XML/sidecar files, rejects legacy `colr.007` rail
  source files unless `--allow-legacy-colr007` is set for local diagnostics,
  rejects zero-record notary anchors before publication and during
  source-file/production evidence replay,
  rejects symlinked receipt archive directories before discovery, and rejects
  repeated receipt paths or copied receipts with duplicate `receipt_sha256`
  values before emitting a summary. Direct receipt and receipt-directory CLI
  paths reject secret-looking key/value material before those paths can be
  serialized into a
  digest-bound, versioned verifier summary with per-receipt file paths,
  `receipt_sha256` values, and policy flags.
- Added `scripts/iso_operator_canary.py`, a strict JSON-runbook runner that
  executes the rail file-drop adapter, audit notary adapter, and receipt
  verifier as subprocesses. It caps runbook JSON at 64 KiB before parsing,
  requires explicit provider/environment labels,
  rejects secret-looking identifier-style provider/environment labels before
  plan-only output or executed summaries can preserve them,
  rejects unknown runbook keys, rejects surrounding whitespace and control
  characters in runbook strings, rejects present `null` optional path and
  numeric limit fields instead of silently applying defaults, keeps relative
  paths inside the runbook directory, rejects endpoint URLs with credentials,
  params, query strings, fragments, malformed bracketed hosts, overlong URL
  strings, DNS hosts longer than 253 characters, localhost/local-private IP
  literals, known local/private rebinding hostnames, or legacy IPv4 numeric
  notation, or IPv6 transition addresses embedding non-global IPv4 addresses,
  accepts checked-in `operator-canary.bank` template endpoints only for
  `--plan-only` validation and rejects them before non-plan child execution,
  rejects non-plan canary config, rail input/message/receipt-dir, notary
  export/receipt-dir, and explicit verifier receipt paths under checked-in
  `fixtures/iso20022/` artifacts before child execution,
  rejects duplicate endpoint lists and duplicate receipt inputs,
  requires list-valued notary and verify receipt-selector fields to be
  explicitly recorded as arrays under `--require-explicit-policy`,
  redacts bearer-token file arguments in its summary, verifies generated
  receipts by default, bounds each child stage with positive finite
  `--stage-timeout-secs`, records `timed_out` for killed children, drains child
  stdout/stderr through a bounded preview cap instead of retaining unbounded
  output, marks the canary failed if any executed child stdout/stderr preview is
  truncated or if a successful child writes stderr, rejects secret-looking child
  output before any canary summary is written, and writes a single bounded versioned summary JSON for operator evidence
  archives. Summary output paths are preflighted
  before subprocess stages, reject control characters, whitespace, leading-dash
  segments, backslashes, semicolon parameters, empty segments, dot/parent
  traversal, checked-in `fixtures/iso20022/` artifact destinations, symlinked
  existing ancestors, and hard-linked, symlink, or
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
  from changing the release-evidence meaning after digest or policy checks, and
  duplicate-key diagnostics now avoid echoing the repeated key name.
  Secret-looking unknown JSON field names now get label-only unknown-key
  diagnostics across the ISO validators while ordinary unknown-field typos
  continue to list the field names for operator debugging.
  Recursive trust-bundle, receipt, evidence, and readiness secret-material
  scanners likewise report only label-level forbidden-field failures, and
  receipt value secret checks no longer echo the receipt field label that
  carried the rejected material. The raw CLI, compact path/value, child-output,
  and recursive scanners cover bearer tokens, private keys,
  passwords/passphrases, API/access/session keys, client secrets, cookies, and
  Iroha signatures, and they now scan repeated percent-decoded forms so encoded
  or double-encoded secret-looking key/value material is rejected in CLI paths,
  unknown JSON keys, recursive JSON values, compact summary paths, and archived
  or live response previews/errors. Unknown JSON keys with control characters
  also fail with label-only diagnostics across live adapters, operator
  receipts, trust bundles, XSD manifests/catalogs, and archive rollups, and
  recursive archive/operator JSON string scans reject unsafe control characters
  before field-specific replay.
  Rail and notary adapters reject successful
  remote response bodies with those markers or unsafe control characters before
  writing receipts, redact failed remote response previews and receipt error
  strings when upstreams return those markers or unsafe control characters, and
  the receipt verifier rejects archived successful receipts that carry the
  redacted response marker plus previews/errors that still contain
  secret-looking material or unsafe control characters. The shared HTTPS URL
  path validators also reject secret-looking key/value material in literal,
  percent-encoded, or double-encoded path segments before live rail/notary
  delivery, canary planning, receipt verification, trust/evidence ingestion, or readiness
  rollup, and reject raw URL delimiter characters in path segments with the
  same fail-closed posture used for encoded delimiters. They also require URL
  paths to stay printable ASCII, rejecting raw Unicode path characters and
  percent-encoded non-ASCII bytes before those URLs can be archived or used.
  The local
  path/raw CLI/summary-path/artifact-path validators use a
  narrow identifier-style path scanner for composite names such as
  `token-*-secret` and strong key markers. Those local path validators also
  reject raw URI or drive prefixes, malformed percent escapes, and
  percent-encoded control/space, dot/separator, semicolon, URL delimiter, and
  percent bytes before path expansion, summary emission, archive replay, or
  child command construction. Direct local CLI/output/artifact path strings are
  capped at 4096 characters with label-only diagnostics before secret scanning,
  filesystem expansion, summary emission, child command construction, or archive
  replay.
  Secret-looking field-name markers
  also cover hyphenated `private-key` and underscore-form `x_iroha_signature`
  spellings across ISO validators, and receipt JSON secret-field scans recurse
  through nested objects and arrays before receipt semantics are evaluated.
  Audit-notary anchor publication also rejects secret-looking audit-index
  identifiers and persisted record-source string values before network
  publication can preserve them in operator evidence, and direct receipt
  verification mirrors that rule when replaying archived notary sources.
  Archived receipt source paths, including rail XML/sidecar paths, notary anchor
  paths, and notary store directories, also reject narrow secret-looking
  identifiers before missing-source or mismatch diagnostics can echo them.
  Notary and receipt replay clean metadata strings from audit indexes,
  persisted records, nullable context/metadata/history fields, and rail sidecars
  are capped at 4096 characters with label-only diagnostics before mismatch,
  source replay, or sidecar validation can retain oversized operator evidence.
  Direct trust-bundle generic strings/OID lists, XSD profile-catalog generic
  strings/lists, canary runbook generic strings/lists, and evidence replay clean
  strings/lists plus production-readiness compact clean strings/lists share the
  same 4096-character label-only cap before trust preflight, XSD profile
  validation, runbook planning, archive validation, or readiness replay can
  preserve oversized metadata; embedded trust/profile DER base64 keeps its
  separate decoded-size guard.
  Direct canary runner, rail-gateway adapter, and audit-notary adapter
  `run(args)` calls also mirror their CLI path-smuggling guards for
  config/summary, inbox/message/receipt/token, and export/receipt/token paths
  before config, inbox, export, or network loading.
  Production-readiness replay also classifies summaries generated from the
  checked-in `fixtures/iso20022/xsd/fixture_manifest.json` corpus as
  `xsd.repository_fixture_manifest` blockers by default, and rejects XSD summary
  input files under checked-in ISO fixture coordinates as
  `xsd.repository_xsd_summary` blockers. Direct XSD `--profile-catalog` inputs
  under checked-in `fixtures/iso20022/` artifacts now fail before manifest
  loading, and archived `profile_catalog.path` values under those artifacts
  replay as `xsd.repository_profile_catalog` blockers. Local
  `--allow-reviewed-xsd-gaps`
  diagnostic runs can only downgrade reviewed corpus warnings.
  Operator evidence verification rejects canary summaries whose `config_path`
  still points at checked-in `fixtures/iso20022/operator_canary/` runbook
  templates, and final readiness replays the compact path as an
  `evidence.repository_canary_config` blocker if a forged aggregate summary
  reintroduces it. The canary runner itself also rejects non-plan
  config/stage/explicit verifier receipt paths under `fixtures/iso20022/`,
  while keeping those templates available for `--plan-only` validation.
  Evidence replay mirrors that rule for executed and planned canary child-command
  `--inbox-dir`, `--message`, `--export-dir`, `--receipt-dir`, and `--receipt`
  values so forged archived commands cannot reintroduce repository fixtures.
  Direct receipt verification and its parent evidence gate also reject
  `--receipt` and `--receipt-dir` selectors under checked-in
  `fixtures/iso20022/` artifacts before receipt discovery, child verifier
  launch, file loading, or digest-bound summary construction.
  Raw evidence verification also rejects `--canary-summary` and
  `--trust-summary` inputs under checked-in `fixtures/iso20022/` artifacts, and
  final readiness blocks forged compact XSD/evidence/canary/trust summary paths
  that point back to those artifacts.
  Operator evidence verification also preserves trust-bundle source paths in
  compact trust profiles and rejects checked-in
  `fixtures/iso20022/trust_bundles/` template paths; final readiness replays
  forged compact paths as `trust.repository_trust_bundle` blockers.
  Rail receipt verification preserves the source XML path as compact
  `source_path` evidence and rejects checked-in `fixtures/iso20022/*.xml`
  fixture paths; evidence and final-readiness replay reject forged compact
  receipt source paths that point back at repository XML fixtures, and the rail
  gateway adapter rejects checked-in ISO XML fixture inputs before network
  delivery or receipt output.
  Notary receipt verification also preserves compact `anchor_path`,
  `store_dir`, and `index_path` evidence, requires the anchor path to remain
  either `latest.notary.json` or `anchors/<index_sha256>.notary.json`, requires
  the index path to stay the `messages.index.json` peer of that anchor export,
  and evidence/readiness replay includes all three values in direct archive
  metadata matching so copied summaries cannot strip or drift the operator
  notary preimage path, source store, or exported audit index while keeping
  matching digests; raw receipts reject notary anchor/store paths under
  checked-in `fixtures/iso20022/` artifacts, and compact replay rejects
  anchor/store/index paths under those artifacts. The audit notary adapter
  rejects checked-in notary anchor/store fixture inputs before network delivery
  or receipt output.
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
  normalization or file discovery, and the canary, XSD, evidence, and readiness
  summary gates also reject secret-looking key/value material in CLI summary
  input/output paths before those paths can be serialized into digest-bound
  rollups. Production-readiness direct `run(args)` calls now mirror the same
  path-smuggling guard for XSD summaries, evidence summaries, and summary
  outputs before input loading while preserving checked-in fixture summary
  inputs as structured release blockers; direct XSD and trust verifier
  `run(args)` calls do the same for manifest/profile-catalog, bundle,
  profile-output, and summary-output paths before manifest or bundle loading.
  Summary/profile outputs and rail/notary receipt output directories
  also reject checked-in `fixtures/iso20022/` artifact destinations during
  run-level preflight and again before creating parents or writing temporary
  output files, so those destinations fail before input loading, child stages,
  schema/trust validation, or network delivery. Live rail/notary adapter runs
  also reject inbox/export roots under checked-in `fixtures/iso20022/`
  artifacts before directory discovery, anchor parsing, XML fixture parsing,
  child execution, or network delivery.
  Canary runbook artifact paths now apply the same narrow
  control-character plus key/value and identifier-style secret-material rejection
  before plan-only summaries or child command arguments are built, while
  bearer-token file paths remain runtime secret-file references and are
  redacted from planned command output. Canary child stdout/stderr previews
  also reject identifier-style secret-looking material and unsafe control
  characters before summary emission.
  Canary runbook path strings and archived child-command local path values must
  also remain printable ASCII and capped by the 4096-character local path limit
  before planning or evidence replay, while production-readiness compact
  summary/config/receipt path strings keep the stricter 2048-character archive
  cap before release archival.
  XSD `xmllint` diagnostics redact key/value secret material,
  identifier-style secret-looking validator output, unsafe control characters,
  and non-ASCII material before reporting schema-validation failures.
  Direct evidence receipt-verifier failures also redact key/value and
  identifier-style secret-looking stderr plus unsafe control characters before
  reporting child verifier diagnostics.
  Duplicate receipt, canary summary,
  evidence summary, and canary-runbook path errors report only the duplicate
  field labels, not the raw duplicated path values. Canary runbook endpoint URL
  and trust-bundle source URL validation errors also report field-label
  failures without echoing rejected secret-bearing URL values, and archived
  canary child commands that contain token-like URL material are rejected by the
  evidence secret scanner without reflecting the command value. Compact path
  strings in archived evidence/readiness rollups, including canary/trust
  summary paths, canary config paths, receipt paths, and XSD schema/fixture
  paths, reject secret-looking key/value material and path confusables before
  successful summaries can preserve those strings. Compact trust-source URL validation inside
  archived evidence and readiness rollups follows the same label-only error
  pattern, and URL port parser failures now omit raw parser exception text so
  malformed operator-provided ports cannot be reflected. URL host validation now
  rejects secret-looking hostname labels and non-ASCII raw host labels, and
  non-port parser failures also use label-only diagnostics before malformed URL
  text can be echoed. Compact canary
  provider/environment, evidence policy context,
  trust profile ID/rail/environment, trust embedded-signature policy,
  profile-override policy, trust source authority/version, and archived trust
  DER label values reject secret-looking identifier-style strings without
  echoing them. Direct trust-bundle material and archived evidence replay also
  require DER labels to be printable ASCII before summaries can preserve
  Unicode-confusable material. Trust embedded-signature policies also reject non-ASCII
  confusable spellings before unsupported-policy diagnostics or readiness
  blockers can preserve forged values, and trust source authority/version
  provenance must be printable ASCII before direct summaries, evidence replay,
  or readiness rollups can preserve it. Trust-bundle SHA-256 pins, declared DER digests, and certificate
  policy OIDs, plus archived evidence/readiness SHA-256 fields for trust bundles,
  profile-override pins, and receipt payload/anchor/index digests, reject the
  same markers before canonical-format diagnostics. Canary runbook
  provider/environment labels enforce that
  identifier boundary before plan-only or executed summaries can persist those
  labels. Live rail sidecar `profile` and `rail_message_id` identifiers
  and archived rail receipt identifiers enforce the same no-echo check before
  receipt emission, network delivery, or summary rollup. Live rail sidecar
  `message_type` values must also remain printable ASCII and match the
  canonical lowercase ISO family-id shape before unsupported-type diagnostics
  can print a short unsupported family value, and rail sidecar
  `message_type`/`payload_sha256` plus archived rail receipt `message_type`
  values reject secret-looking markers before digest or summary diagnostics.
  Direct receipt replay, evidence replay, readiness replay, and XSD profile
  catalogs now use ASCII-only rail message-type digits, and evidence/readiness
  archive/canary kind, filename, or metadata mismatch blockers avoid printing
  receipt kind values, receipt leaf names, or full metadata tuples.
  XSD profile-catalog enum values such as rails, embedded signature policies,
  reference datasets, and structured-address modes also reject overlong ASCII
  spellings before unknown-value diagnostics can print them.
  Profile-catalog profile IDs are capped before duplicate-ID or
  missing-schema-version diagnostics can print them.
  Profile-catalog business-service entries are capped before overlong service
  identifiers can be emitted or archived.
  XSD/XML schema and fixture identifiers are capped before schema/root mismatch
  diagnostics can print overlong ASCII payload names or namespace URIs.
  XSD profile-catalog `message_def_id` and version entries also require
  ASCII-only digits before missing-schema or skipped-version classification.
  Receipt verifier, evidence, and readiness `receipt_kind` values reject
  secret-looking identifier markers and non-ASCII confusable spellings before
  unsupported-kind diagnostics or blockers can preserve forged archive values,
  and archived canary stage names reject those markers plus non-ASCII confusable
  spellings before unsupported-stage, ordering, or stage-window diagnostics.
  Trust-bundle preflight, evidence replay, and production-readiness compact
  trust profile IDs, override IDs, embedded signature policy strings, and
  trust-source authority/version/timestamp provenance are capped before trust
  diagnostics can print or archive them.
  Live rail/notary adapter timeouts must be
  positive finite numbers, and their response/payload byte caps must be positive
  integers rather than JSON/Python boolean aliases before any local read or
  network delivery. All ISO helper regular-file byte caps, plus the rail
  payload bounded reader, reject boolean and non-integer limit aliases before
  file metadata is inspected. Operator receipt, summary,
  and emitted profile-override output paths also reject control characters, whitespace,
  leading-dash segments, backslashes, semicolon parameters, dot/parent
  traversal, empty segments, symlinked existing ancestors, and hard-linked,
  symlink, or non-regular targets before writing, then replace the target
  atomically from an owner-private descriptor-checked temporary file with a bounded
  digest-derived name.
  Direct ISO CLI path preflights treat bare path flags, empty separate values,
  empty equals-form values, separate values that begin with `--`, and
  equals-form values such as `--summary-out=--flag` as missing path values, so
  flag tokens cannot be misparsed as operator-local paths before file or network
  work starts. Required notary/rail URL values now also reject control-bearing,
  non-ASCII, whitespace-padded, or non-URL-shaped secret-looking values before
  unrelated file or directory requirements can mask the URL error. Direct numeric CLI preflights
  similarly reject malformed, empty, flag-looking, secret-looking, and non-ASCII
  numeric values before Python/argparse can accept Unicode digit confusables or
  echo raw operator input. All ISO operator entry
  points reject secret-looking raw CLI
  tokens, including percent-encoded forms, before argparse can echo unknown
  arguments, and reject control-bearing unknown CLI tokens before terminal
  control bytes can reach argparse diagnostics. Unknown raw CLI tokens must also
  be printable ASCII, so Unicode-confusable option spellings cannot reach
  argparse output. Because those tools do not accept positional operands, they also
  reject the `--` argument terminator in raw secret, boolean, path, context, and
  numeric preflights before any trailing flag/value material can bypass scanning
  and be echoed by argparse. The parsers also disable long-option abbreviation,
  so partial spellings such as `--summary-ou` cannot be accepted as exact
  production flags. Direct boolean CLI
  flags reject attached `--flag=value`
  spellings and separate non-option values before argparse can echo the value or
  reinterpret the option. Evidence and production-readiness
  provider/environment context flags reject missing, empty, flag-looking,
  secret-looking, or non-ASCII values before argparse or summary loading, while
  canary and trust-bundle context summaries must keep provider/environment
  labels printable ASCII. Expected provider/environment mismatch diagnostics
  remain label-only and do not print observed or expected context values.
  Archived canary command replays reject unsupported secret-looking or
  non-ASCII flag names with label-only diagnostics before evidence summaries can
  echo the flag spelling.
  Unknown JSON field names that are secret-looking, control-bearing,
  non-ASCII, overlong, too numerous, or collectively oversized are likewise
  reported with label-only diagnostics while ordinary ASCII typos remain listed
  for operator ergonomics.
  Duplicate record, list, digest, OID, DER, compact-summary, archived
  receipt-reuse, and trust-material diagnostics now report label/index-only
  structural failures without echoing the rejected duplicate value.
  Version-bearing manifests, trust bundles, notary audit indexes, notary
  anchors, persisted record sources, and adapter receipts require exact integer
  version values and reject JSON booleans. Receipt `status_code` metadata also
  rejects JSON booleans before success-policy checks. Notary audit-index,
  anchor, and receipt `record_count` metadata reject JSON boolean aliases
  before count equality or source-binding checks. Notary audit-index records
  must carry canonical Torii lifecycle states (`Pending`, `Accepted`, or
  `Rejected`), state-compatible pacs.002 summary codes, and the complete
  summary key set emitted by Torii, including nullable fields such as
  `settled_at_ms`, `transaction_hash`, `uetr`, and `reference_snapshot_id`, so
  omission cannot be treated as intentional null evidence during source replay,
  and notary publication plus production receipt evidence must bind a positive
  notary `record_count` instead of an empty anchor. Persisted notary record
  sources must likewise
  carry the complete record, context, metadata, and status-history key sets
  emitted by Torii during source replay and Torii durable-store reload while
  still allowing intentional JSON nulls for nullable fields, requiring each
  history entry's pacs.002 code to match its lifecycle status, and requiring
  concrete persisted strings to be non-empty, unpadded, and control-free; Torii
  reload binds every accepted record to the digest-addressed filename derived
  from its embedded `message_id` without following symlinked `messages`
  directories or record paths, or reading records larger than 1 MiB; Torii also
  omits oversized runtime records from durable files and regenerated audit
  indexes. Canary, evidence, and XSD
  bounded subprocess output limits reject boolean and non-integer aliases before
  child commands are run or plan summaries are emitted.
  Canary runbook paths reject embedded whitespace, leading-dash path segments,
  backslashes, semicolon path parameters, empty segments, and dot/parent
  traversal segments before path expansion or child-script planning.
  XSD manifest/profile optional fields are optional only when omitted; explicit
  `null` values for schema/fixture reviewed reasons, trust and revocation
  material lists, booleans, numeric caps, business-service arrays, and amount
  minor-unit arrays now fail before digest-bound XSD summaries are emitted.
  Reviewed XSD gap reasons and profile-catalog identity strings also reject
  ASCII control characters at preflight time. Reviewed gap reasons and
  blocked-source review reasons must also remain printable ASCII and
  secret-looking-free with a 1024-character cap in direct XSD summaries and
  production-readiness replay, and production readiness rejects digest-correct
  archived XSD summaries whose reviewed gap reasons are present but empty,
  non-string, or overlong, or whose copied missing-schema/schema-only gap-list
  entries carry non-canonical paths or reviewed reasons, or whose
  schema-backed fixtures still carry a missing-schema reason. Rejected XSD manifest and archived summary path
  validation errors report label-only failures without echoing raw path values
  that may contain secret-looking segments. Checked-in XSD source provenance
  now rejects placeholder GitHub repository coordinates plus non-ASCII or
  overlong source paths, embedded whitespace, semicolon path parameters,
  identifier-style secret-looking path material, and secret-looking repository coordinates during preflight,
  and production readiness replays the same repository-coordinate rejection
  before emitting archived XSD summaries.
  Archived profile-catalog paths, including checked-in fixture coordinates, get
  the same readiness recheck when production readiness consumes archived XSD
  summaries.
- Added `scripts/iso_operator_evidence_verify.py`, an offline production
  evidence gate for archived canary and trust-bundle summaries. It caps each
  input JSON file at 4 MiB before parsing, recomputes summary digests, requires
  exact expected provider/environment CLI context,
  rejects missing or unsupported canary and trust-bundle summary versions,
  requires successful rail/notary/verify stages by default,
  requires the verify stage to carry digest-bound receipt-verifier JSON with
  rail and notary receipt kinds plus per-receipt digests, caps direct
  receipt-verifier stdout/stderr at 4 MiB before JSON parsing, bounds direct
  receipt-verifier runtime with positive finite
  `--receipt-verifier-timeout-secs`, redacts secret-looking or control-bearing
  direct receipt-verifier stderr before failed child verifier diagnostics
  report a detail, rejects missing or unsupported
  receipt-verifier summary versions, rejects plan-only
  or dry-run canaries, plaintext-HTTP overrides, default-profile fallbacks,
  legacy `colr.007` local overrides, unredacted bearer-token paths,
  non-canonical compact receipt paths with control characters, surrounding
  whitespace, dot or parent traversal segments, or non-`*.receipt.json` leaves,
  secret-looking output, non-canonical canary runbook `config_path` values with
  control characters, surrounding whitespace, traversal segments, or non-JSON
  leaves, control-bearing, whitespace-padded, or secret-looking provider, stage,
  receipt-kind, trust-profile, trust source, or archived trust DER label
  identity strings, unknown raw canary, receipt-verifier, or trust-bundle summary
  fields,
  smuggled trust-source URLs including repeated path separators, missing/malformed/future
	  trust-source retrieval timestamps, missing/malformed/future or padded
	  trust-summary `verified_at` timestamps, smuggled child command endpoint URLs
	  including overlong URLs, invalid or overlong hosts, repeated path separators,
	  localhost/local-private IP literals, known local/private rebinding hostnames,
	  legacy IPv4 numeric notation, IPv6 transition addresses embedding non-global
		  IPv4 addresses, reserved documentation hosts such as `.example`,
		  `example.com`, `example.net`, `example.org`, or `example.invalid`,
		  checked-in template hosts under `operator-canary.bank` in direct
		  receipt archive replay, and percent-escape smuggling, local-only
  child command flags in either `--flag` or `--flag=value` form, including the
  notary adapter's `--allow-missing-record-sources` diagnostic override,
  unsupported child command flags outside the expected rail/notary/receipt
  verifier CLI surfaces, duplicate singleton child command flags, boolean
  child command flags using attached or separate values, non-positive or
  non-finite numeric child command flag values, Unicode digit confusables in
  floating timeout flags, value-taking child command flags whose separate or
  equals-form values are empty or another flag token,
  child command arrays that do not start with a Python interpreter using
  ASCII-only numeric version suffixes plus the expected stage script path, or
  that contain extra positional arguments after that runner-emitted prefix,
  non-ASCII or non-canonical child command path values including leading-dash segments,
  control characters or surrounding whitespace in executed or plan-only child
  command arrays,
  and child command arrays that omit required stage inputs,
  non-canonical rail/notary `receipt_dir` values with control characters,
  surrounding or embedded whitespace, leading-dash path segments, semicolon path
  parameters, empty segments, raw backslashes, or traversal segments,
  rail/notary `receipt_dir`
  values that do not match the child command's single `--receipt-dir`,
  verify-stage commands that omit the rail/notary receipt directories generated by non-dry-run stages,
  synthetic trust DER, record-only trust policy, and placeholder trust-source
  provenance such as `dummy`, `fake`, `placeholder`,
  `replace-before-production`, `sample`, `template`, or reserved hosts such as
  `.example`, `example.com`, `example.net`, `example.org`, and
  `example.invalid` before an archive is treated as production evidence.
  Direct evidence summary input/output CLI paths and compact archived path
  values reject secret-looking key/value material before an evidence summary is
  emitted, so operator-local path strings cannot smuggle runtime tokens into
  release archives.
  For archived trust-bundle summaries, it preserves the
  bundle-level `bundle_sha256`, rejects duplicate compact receipt paths/digests,
  duplicate bundle digests, and duplicate profile IDs with label-only
  diagnostics, requires emitted profile overrides to match the
  top-level profile id, rail, and signature policy, rechecks override pin/OID
  and CRL/OCSP material counts against the verifier's material summary, binds
  CRL/OCSP override DER back to recorded summary digests and byte lengths,
  replays the CRL-vs-OCSP material-class checks on the archived override DER,
  rejects malformed policy OIDs or non-canonical, oversized, or non-SEQUENCE
  CRL/OCSP base64, and fails closed on same-count trusted/revoked pin overlap
  attacks.
- Added `scripts/iso_production_readiness.py`, an offline release-readiness
  rollup for versioned digest-bound XSD and operator evidence summaries. It requires
  XSD/evidence summary JSON inputs to stay within a 4 MiB pre-parse cap,
  exact provider/environment CLI context, strict schema-backed/fixture-backed
  XSD proof with canonical fixture schema references and reviewed-gap reason
  strings, production evidence policies,
  per-canary explicit-policy proof, full rail/notary/verify canary evidence,
  versioned digest-bound direct receipt-archive
  verification with per-receipt digests, rail/notary receipt kinds, no legacy
  `colr.007` local overrides, no default rail profile fallback, positive
  notary record-count proof, and
  `require-verified` trust profiles with at least one public-key or X.509
  trust pin plus required CRL/OCSP revocation
  checks and material before reporting the ISO corridor ready. It also
  recursively rejects secret-looking fields or string values in archived XSD
  and evidence summaries, including compact path strings and compact
  provider/environment/trust-source identity values, before emitting the final
  readiness rollup, preserves normalized XSD blocked-source evidence in
  the public readiness summary, rejects blocked-source records that no longer
  correspond to a current missing schema/profile gap, and rejects replayed
  blocked-source references or candidate digests across repeated XSD summaries.
  Readiness replay also rechecks that blocked-source marker lists contain
  explicit redistribution restriction evidence rather than copyright-only
  provenance.
- Added checked-in operator canary runbook templates under
  `fixtures/iso20022/operator_canary/` for Swift CBPR+, Fedwire Funds, SEPA SCT
  Inst, and securities CSD profile families. They use
  `operator-canary.bank` template endpoints for plan-only validation; non-plan
  canary execution and archived production evidence both reject that template
  suffix before it can be treated as live evidence.
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
  emitting a versioned trust summary, rejects repeated input paths without echoing the
  raw path value, rejects secret-looking bundle/output CLI path material before
  summary or profile-override emission, rejects JSON boolean bundle versions,
  copied bundles, and duplicate profile
  IDs, emits and de-duplicates bundle-level `bundle_sha256` values, enforces
  supported `embedded_signature_policy` values (`record-only`,
  `reject-unsupported`, or `require-verified`) before allowing local
  record-only diagnostic replay,
  unique DER labels per material class, requires
  DER-object `sha256` values before digest matching
  or profile override emission, omits absent DER labels from trust summaries,
  and rejects secret-looking fields or string values such as `token=...`,
  `Authorization: ...`, `private_key=...`, or `X-Iroha-Signature: ...` before
  emitting Torii profile trust overrides. Trust-bundle profile IDs,
  environments, source authority/version strings, DER labels, and recursively
  scanned field names also reject secret-looking identifier-style values before
  summary or profile-override emission. `--max-source-age-days` rejects
  missing, empty, flag-looking, malformed, or secret-looking freshness budgets
	before argparse or bundle reads.
- Trust-bundle local-audit overrides now fail closed unless they are actually
  needed by the verified input: `--allow-record-only` requires a non-production
  `embedded_signature_policy`, `--allow-insecure-source-url` requires an
  `http://` source URL, and `--allow-synthetic-der` requires DER material that
  fails the expected certificate/CRL/OCSP shape check. The private synthetic-DER
  usage marker is stripped from emitted summaries.
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
`--require-explicit-policy`, which requires every runbook policy boolean and
the notary/verify list-valued receipt selector fields to be present, then
records that proof in the summary for the evidence gate. Verify receipt
selectors must be unique and non-overlapping at planning time, including direct
`verify.receipts` files that are already covered by explicit or generated
`verify.receipt_dirs`; regression coverage removes each rail, notary, and
verifier policy boolean or explicit list in turn. Checked-in
templates for Swift CBPR+, Fedwire Funds, SEPA SCT Inst, and securities CSD
live-profile families live under
`fixtures/iso20022/operator_canary/`; copy them into an operator runbook area
before replacing template endpoints, token-file paths, inboxes, and
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
provenance, explicit top-level `source` objects with omitted and null source
provenance rejected separately, non-placeholder production source metadata and source URLs that do
not use reserved placeholder hosts such as `.example`, `example.com`,
`example.net`, `example.org`, `example.invalid`, or
`operator-canary.bank` in production
evidence/readiness gates, and no runtime secret fields or secret-looking string values. DER-object `sha256`
values are mandatory and must match canonical decoded `der_base64` bytes for
trust anchors, revoked certificates, CRLs, and OCSP responses. DER labels are emitted
only when present and non-empty; archived summaries that explicitly carry
`label: null` fail production-evidence verification. Trust bundles must record
every list-typed material field as an array, including explicit `[]` values for
intentionally empty pin or DER collections, so omitted trust material cannot
silently change profile overrides. Production bundles
also need DER values that look like the expected material class; the checked-in
templates use synthetic DER envelopes only so CI can validate schema and
emitted-profile wiring, and require `--allow-synthetic-der`.
Synthetic-template validation is summary-only:
`--allow-synthetic-der` cannot be combined with `--emit-profile-json`. Replace
templates with the current rail PKI package before production use and omit that
flag before emitting profile overrides. Trust-bundle preflight also rejects
unused local-audit override flags: `--allow-record-only` must correspond to a
non-production `embedded_signature_policy`, `--allow-insecure-source-url` must
correspond to an `http://` source URL, and `--allow-synthetic-der` must
correspond to DER material that fails the expected certificate/CRL/OCSP shape
check. The private synthetic-DER usage marker is not emitted in summaries.
Profile emission also refuses
  `--allow-record-only`, `--allow-insecure-source-url`, placeholder
  authority/version strings including `dummy`, `fake`, `sample`, or `template`,
  reserved placeholder source URLs such as `.example`, `example.com`,
  `example.net`, `example.org`, `example.invalid`, or
  `operator-canary.bank`, missing source freshness budgets, and
  stale source retrieval timestamps.
Trust bundles must carry an explicit `embedded_signature_policy`; the preflight
does not infer `require-verified` from an omitted policy field.
Example trust preflight command:

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
The aggregate evidence gate accepts `--allow-profile-json-not-emitted` only when
at least one archived trust summary actually records
`profile_json_emitted=false`, so an unused diagnostic override cannot be
preserved as a non-production policy bit beside otherwise emitted profile JSON.
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
summary also preserves the trust verifier's local-only
`allow_synthetic_der`, `allow_record_only`, and `allow_insecure_source_url`
flags, rechecks that `profile_json_emittable` matches those flags plus the
  archived trust source policy, rejects omitted compact `source` keys, preserves
  explicit diagnostic `source: null` profiles only when missing-source replay is
  allowed, and preserves
`profile_json_sha256` so release evidence names the exact profile override body
produced by the trust preflight; the evidence gate recomputes it from the
archived profile override objects before archival.
Repeated canary/trust summary paths or copied summaries with the same
`summary_sha256` are malformed input. By default it requires
digest-bound, successful canary summaries with rail, notary, and
receipt-verify stages in canary-runner order;
rejects forged canary summaries that carry both executed `stages` and
plan-only `planned_stages` branches before compacting stage evidence;
requires the canary summary to prove it was generated with
`--require-explicit-policy`;
requires the receipt-verify stage output to be digest-bound and contain
positive receipt counts, duplicate-free rail/notary receipt-kind lists, and
per-receipt `receipt_sha256` and `response_body_sha256` entries with unique
receipt paths and unique receipt digests, explicit `ok=true` plus 2xx
`status_code` metadata on every receipt entry, kind-specific notary
anchor/index/count metadata or rail
message/profile/payload metadata whose rail message type is in the adapter
endpoint map, and explicit boolean receipt policy fields rather than omitted
defaults;
rejects timed-out stages, non-null successful-stage `reason` fields, truncated
child-process stdout/stderr previews, unsafe control-character or
identifier-style secret-looking preview content, and non-empty successful
stderr previews from every executed canary stage before
trusting the archived stage output;
requires timeout-bounded direct receipt archive verification from `--receipt` or
`--receipt-dir` before emitting an evidence summary, rejects successful direct
receipt-verifier stderr instead of dropping hidden diagnostics, and requires that direct archive's
`receipt_sha256` entries cover every canary receipt-summary digest with the same
receipt filename, receipt kind, successful status metadata, response-body
digest, endpoint-policy evidence, and kind-specific receipt metadata; the local canary-stage-only
diagnostic override is rejected
when any direct `--receipt` or `--receipt-dir` archive input is supplied, so
  direct replay evidence cannot carry that non-production policy bit; direct
  receipt verification rejects rail receipts that omitted
  an explicit profile unless `--allow-default-profile` is supplied, and that local
  override is preserved as an explicit receipt-summary policy flag;
requires every non-legacy `iso-rail-gateway` canary receipt profile to have
matching compact trust material for the same profile ID and environment before
the archive is accepted; built-in rail-named profiles must also bind to the
same rail in the trust profile;
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
CRL/OCSP DER material-class checks plus digests and byte lengths that agree with the trust-bundle verifier
summary; and scans
archived commands/output for obvious secret leakage.
Plan-only diagnostic archives must still record each planned stage's `dry_run`
boolean, and that boolean must match the planned child command's `--dry-run`
flag. The evidence gate accepts `--allow-plan-only` only when at least one
archived canary summary records `plan_only=true`. Bearer-token file
arguments must be redacted whether represented as
`--bearer-token-file <path>` or `--bearer-token-file=<path>`. The `--allow-*`
  flags, including `--allow-legacy-colr007`,
  `--allow-missing-record-sources`, `--allow-canary-stage-receipts-only`, and
  `--allow-profile-json-not-emitted`, are for local test audits only and should not
  be present in production evidence archives. The live rail and notary adapters
  reject unused local `--allow-insecure-http`, `--allow-default-profile`, and
  `--allow-legacy-colr007` flags before dry-run summaries or network delivery,
  and the notary adapter rejects unused `--allow-missing-record-sources` unless
  at least one validated anchor actually lacks local record sources.
  The evidence gate also rejects
`--allow-partial-canary` unless at least one archived canary summary is missing
a rail or notary stage, rejects unused legacy/default-profile receipt overrides
unless compact rail receipts actually carry legacy `colr.007` or missing
profile evidence, and rejects unused record-only/synthetic/missing-source trust
overrides unless compact trust summaries carry the corresponding diagnostic
trust material; compact record-only and insecure-source trust policy flags must
also bind to a non-production `embedded_signature_policy` or an `http://` or
local/private source URL in the same trust summary rather than a summary flag
alone. It also rejects
unused dry-run, failed-receipt, and
insecure-HTTP diagnostic overrides unless the archived canary command actually
targets HTTP or local/private routing, or the receipt summary or trust summary
actually carries that policy, and rejects unused receipt-source-missing
overrides unless a verified receipt summary records
`require_source_files=false`; failed-receipt policy must come from a receipt
summary with at least one failed receipt entry rather than planned command text
or a summary flag alone. Executed rail/notary child commands whose endpoint URL
needs insecure/local policy must carry `--allow-insecure-http` on that same
child command, and the captured receipt summary must carry
`endpoint_requires_insecure_http=true` for the matching compact receipt kind.
Executed rail commands that use `--allow-default-profile` or
`--allow-legacy-colr007` must likewise match compact rail receipt evidence for
a missing profile or legacy `colr.007` message type, and those compact receipt
conditions are rejected if the rail command omitted the corresponding flag.
Executed canary stage names also bind to compact `receipt_kind` evidence:
rail/notary stages require matching `iso-rail-gateway` or `iso-audit-notary`
receipt kinds, and partial canary receipt summaries cannot include receipt kinds
for stages that were not present in the archived run. The verify-stage
`--receipt-dir` list is scoped the same way for executed and plan-only canary
branches: every non-dry-run rail/notary receipt dir must be present, and extra
receipt dirs that do not belong to a recorded rail/notary stage are rejected;
direct `--receipt` files must likewise live under one of the recorded stage
receipt directories. Rail and notary stages must not share a receipt directory,
and the verify stage's own `receipt_dir` field must be null or omitted.
Verify-stage receipt selectors must also be unique and non-overlapping, so
duplicate `--receipt-dir` or `--receipt` values and direct receipt files already
covered by a selected receipt directory are rejected before receipt-verifier
stdout is trusted.
The canary verify stage
must bind its recorded receipt-verifier command flags to the captured
receipt-verifier JSON policy booleans, so diagnostic policy cannot be hidden in
stdout or invented after execution. The production-readiness replay gate mirrors
those bindings for compact evidence summaries, so forged `allow_failed=true`,
legacy `colr.007`, or default-profile policy flags without matching receipt
entries are reported as invalid diagnostic evidence. Even when local diagnostic
`--allow-insecure-http` replay is enabled, archived child-command URLs still
reject reserved documentation hosts and the checked-in `operator-canary.bank`
template suffix.

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
non-ASCII or invalid host labels, percent-host and percent-path smuggling including encoded
semicolon parameters and encoded URL delimiters, numeric-host
spoofing, repeated path separators, and path traversal, rejects omitted,
malformed, evidence-weaker, or release-weaker compact trust source freshness
budgets, rejects compact `profile_json_emittable` values that no longer match
the archived source provenance and freshness budget, rejects compact summaries
that report emitted profile JSON while not emittable, rejects compact
canary/trust summary paths,
canary config paths, receipt paths, and rail receipt `source_path` XML paths
with embedded whitespace, semicolon path
  parameters, leading dashes, leading-dash path segments, empty segments, raw
  backslashes, or dot/parent traversal, rejects repeated XSD/evidence
summary paths or copied summaries with the same `summary_sha256`, and rechecks
digest-bound XSD `schemas[]` and
`fixtures[]` arrays for count consistency, unique schema and fixture evidence
digests, cross-summary schema/fixture path and digest replay, normalized
blocked-source evidence, blocked-source-to-current-gap consistency, plus
blocked-source replay across repeated XSD summaries, blocked-source
redistribution marker strength, canonical relative schema paths whose filenames match
`message_def_id`, fixture paths that remain printable-ASCII relative
forward-slash XML paths no longer than 2048 characters and without leading
dashes, URI/drive prefixes, malformed or smuggled percent escapes, empty, dot,
or non-leading parent segments, schema `target_namespace` binding to
`message_def_id`, fixture schema-reference message-id and payload-root binding,
schema-backed/missing-schema consistency, and schema-backed fixture XML
schema-validation proof. It also rechecks archived schema/fixture XML
identifier strings with the same printable-ASCII, secret-looking-material, and
256-character caps as the direct XSD verifier, profile-catalog source digests,
version coverage counts, canonical profile ids, ISO family message types,
allowed directions, message-definition family binding, and missing-version
entries, plus skipped family-version alias canonicality and profile-catalog
`profiles` count consistency against represented profile IDs, rejects unknown
XSD summary fields across strict flags,
schema/fixture/gap/profile-catalog entries,
and the XSD preflight rejects unknown keys in source profile/message catalog
entries plus present-null optional manifest/profile fields before such fields
can reach the summary,
while readiness rejects archived XSD summaries that omit the emitted
`manifest` path or `profile_catalog` key; explicit `profile_catalog: null`
remains recorded diagnostic no-catalog evidence and cannot satisfy the strict
profile-schema-backed gate,
recomputes `profile_catalog.missing_schema_versions`, and cross-checks
schema-only flags and reviewed reasons against the schema/fixture relationship.
Reviewed `missing_schema_fixtures` and `schema_only_entries` must also match the
recomputed exact path, `message_def_id`, and reason tuples.
It blocks summaries that do not prove `--require-profile-schema-backed-versions`;
the direct XSD verifier uses the default profile catalog for that strict flag
when no override is supplied. The
rollup rechecks the direct receipt archive verification emitted by the evidence
gate from `--receipt` or `--receipt-dir`, not only the canary stage's captured
verifier stdout, and blocks digest-correct evidence summaries whose direct
archive receipt digests no longer cover the canary receipt-summary digests or
whose direct archive carries receipt digests that no canary references, or whose
canary entries relabel the archived receipt filename or kind, or drift from the
archived successful status, response-body digest, endpoint-policy evidence, or kind-specific receipt
metadata. Distinct canary summaries
must not reuse compact receipt paths or receipt digests. The evidence gate also
rejects rail/notary source paths or source digests replayed across distinct
canary summaries inside one aggregate evidence summary, and final readiness
aggregation blocks the same source-material replay across distinct evidence
summaries even when the receipt paths and receipt digests have been relabelled.
That
archive verification summary must carry its own
`summary_sha256`, production policy flags, and a `receipts[]` list binding each
unique canonical `*.receipt.json` path to its unique `receipt_sha256` and one
of the supported rail/notary receipt evidence kinds while preserving
`ok=true`, 2xx `status_code` success metadata, endpoint-policy evidence, and the corresponding
kind-specific notary or rail compact metadata. The direct receipt verifier
rejects unused local policy flags before that summary can be archived. Evidence
summaries that use the
local canary-stage-only diagnostic path must still record
`receipt_verification: null` and must retain
`policy.allow_canary_stage_receipts_only: true`; omitting the archive key or
forging the policy flag back to `false` remains a blocker even under local
readiness replay, the evidence gate rejects combining the policy flag with
direct receipt archive inputs, and the readiness policy flag is still blocked
when a forged direct receipt archive summary is present. XSD
strict-mode flags, evidence-level production policy flags, and nested
receipt-summary policy flags must all be present as booleans; evidence `ok`,
canary `plan_only`, and
per-canary `require_explicit_policy` status fields are also required booleans.
Trust-summary verifier flags are enforced by the evidence gate before the rollup
accepts an archive, retained in the compact summary, and independently blocked
by readiness if a replayed summary still carries synthetic DER, record-only
policy, or insecure source-URL allowances. The rollup also rechecks compact trust-profile
`bundle_sha256`, CRL/OCSP revocation booleans, material counts, revoked and
certificate-policy counts, compact DER proof shape, count binding, and
cross-role digest reuse, and
`verified_bundles` profile-count binding. Omitted compact trust-profile
`source` keys remain malformed, while explicit `source: null` diagnostic replay
is retained and reported as `trust.source_missing` instead of aborting the
release report. Compact trust summaries must also
retain `profile_json_emitted=true`, `profile_json_emittable=true`, and a
lowercase `profile_json_sha256`; false values, missing or malformed profile JSON
digests, duplicated compact trust-profile IDs, or duplicated bundle digests are
production blockers, and `profile_json_emittable` is recomputed from the compact
source evidence before the rollup is accepted. The rollup also binds canary
rail evidence to trust material by preserving each rail receipt `source_path`,
blocking repository XML fixture sources, preserving notary receipt
`anchor_path`, `store_dir`, and `index_path` values in direct archive matching,
and requiring every `iso-rail-gateway` receipt profile exercised by a canary to
have a matching compact trust profile for the same profile ID and environment;
built-in rail-named profiles must also bind to the same rail in the trust
profile.
Evidence and readiness blockers for missing
trust coverage identify the canary receipt index without printing the compact
profile ID or canary environment label.
The rollup requires at least one XSD summary and at least
one evidence summary, and compact canary and trust summary entries must also
retain control-free, trim-free, leading-dash-free, traversal-free source paths
that point to `.json` files plus canonical lowercase `summary_sha256` pointers;
nested canary, trust, archive-receipt, and
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
they are missing for executed canaries, malformed, outside the canary window, or
name-mismatched; plan-only compact canaries must instead keep
`stage_windows: []` and explicitly recorded `receipt_summary: null`; omitting
that key is malformed replay input. This prevents forged plan-only evidence
from smuggling executed receipt evidence or execution windows. They also reject
reordered or overlapping stage windows and duplicate or unsupported compact
stage names, including compact stage-name sequences that do not follow
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
contains template markers such as `dummy`, `fake`, `placeholder`,
`replace-before-production`, `sample`, or `template`, or whose source URL still
points at reserved placeholder hosts such as `.example`, `example.com`,
`example.net`, `example.org`, or `example.invalid`, are production blockers even when the DER
material itself is real. Compact trust-source URLs also fail closed on overlong
URLs and DNS hosts instead of relying on downstream URL consumers.
`--allow-reviewed-xsd-gaps` and `--allow-canary-stage-receipts-only` exist for
local diagnostic audits of the current checked-in fixture corpus; production
release evidence should omit them and must make the strict XSD, profile-catalog,
and receipt-archive checks pass. The final readiness gate rejects those local
overrides when they are unused, so `--allow-reviewed-xsd-gaps` must correspond
to at least one reviewed XSD or repository-fixture warning and
`--allow-canary-stage-receipts-only` must correspond to an evidence summary with
canary-stage-only receipt policy or missing direct receipt archive verification.

## Gap Register

| Area | Current state | Target |
| --- | --- | --- |
| Rail connectivity | Local bridge endpoints plus `scripts/iso_rail_gateway_adapter.py`, an operator-side file-drop adapter that verifies sidecar-pinned message type/profile/payload digest and rejects unsupported rail message types, duplicate payload digests, or duplicate rail message ids before submitting to Torii and writing receipts outside consensus-critical code; `colr.012` is the production collateral-substitution family and legacy `colr.007` requires an explicit local override that receipt/evidence/readiness gates reject for production; `scripts/iso_operator_receipt_verify.py` gates the resulting receipts, `scripts/iso_operator_canary.py` ties the adapter plus verifier into a reproducible provider runbook, `scripts/iso_operator_evidence_verify.py` rejects non-production archived summaries, `scripts/iso_production_readiness.py` aggregates accepted summaries into one release gate, and checked-in Swift/Fedwire/SEPA/CSD templates plan successfully without network access | Run provider-specific live gateway canaries for selected SWIFT/Fedwire/SEPA/CSD operators and archive evidence summaries that pass the production-readiness gate |
| XMLDSig/XAdES | Supported P-256/SHA-256 enveloped subset is verified against profile public-key, leaf-certificate, and linked certificate-chain pins with non-CA XMLDSig leaf certificates carrying critical `keyUsage`/`digitalSignature`, deterministic child issuer distinguished-name binding to parent subject distinguished names, bounded duplicate-free `X509Data` certificate chains, certificate-chain ECDSA-with-SHA256/id-ecPublicKey-secp256r1 enforcement, critical issuer CA `basicConstraints` and `keyUsage`/`keyCertSign`, issuer path-length constraint enforcement, rejection for unknown, malformed, or unsupported parsed critical X.509 extensions, extension/validity checks against verified signed XAdES `SigningTime` or BAH `CreDt`, explicit certificate revocation pins, low-S fixed-width `r || s` or low-S DER ECDSA signature values, one empty or unique same-document `#id` payload Reference URI that strictly encloses the verified signature carrier with an enveloped-signature transform first, at most one final supported C14N transform, one optional XAdES `SignedProperties` Reference with a local `#id`, `QualifyingProperties` target bound to the enclosing `Signature` `Id`, exactly one supported bare `Signature` or direct-child `Sgntr`/`Signature` carrier, ordered direct `Signature`/`SignedInfo` child parsing, prefixed XMLDSig structure bound to the XMLDSig namespace, prefixed XAdES structure bound to the ETSI XAdES v1.3.2 namespace, exact QName opening/closing tag matching in supported XML spans, malformed structural QName rejection, direct `Signature/Object/QualifyingProperties/SignedProperties/SignedSignatureProperties` XAdES parsing, certificate-backed `SigningCertificateV2` ordered duplicate-free chain-prefix digest binding with direct `Cert`/`CertDigest` children only, no unreferenced, wrapped, or duplicate `SignedProperties` elements, parameter-free XMLDSig method/transform elements with exact-one critical methods, exact-name method/digest/Reference policy attribute lookup, exact-one attribute-free `Transforms` wrappers, implemented ordinary attributes only, and ordered direct `Reference` children, singleton required base64 values, unambiguous public-key-or-certificate key material scoped to exactly one structured `KeyInfo`, inherited namespace context for referenced roots, a fail-closed supported canonical XML subset for empty-element expansion, simple attribute normalization, namespace-aware attribute sorting, implicit `xml:` namespace attributes, legal `xmlns:xml` declaration omission, XML-character-reference decoding, no-comments C14N comment omission, and C14N-mode-specific root namespace declarations inherited from the enclosing `Signature`; `scripts/iso_trust_bundle_verify.py` preflights operator trust bundles, `scripts/iso_operator_evidence_verify.py` rejects synthetic/record-only trust summaries for production archives, `scripts/iso_production_readiness.py` rechecks production trust posture in the release rollup, and checked-in Swift/Fedwire/SEPA/CSD templates validate schema and emitted trust overrides | Replace synthetic trust-bundle templates with official profile-specific trust-anchor packages, add complete official canonical XML fixture coverage, add official CRL/OCSP or rail revocation-feed fixtures, and archive evidence summaries that pass the production-readiness gate |
| Follow-up messages | Inbound `pacs.002`, `pacs.004`, `camt.056`, `sese.023`, `sese.024`, `sese.025`, and `colr.012` lifecycle endpoints record durable messages, reject replay evidence, and update known referenced originals only; checked-in payment, securities, and collateral XML fixtures now cover `pacs.002`, `pacs.004`, `camt.056`, `sese.023`, `sese.024`, `sese.025`, and `colr.012` profile/lifecycle handling; the offline MDR/XSD fixture matrix now includes standalone fixtures for every checked-in payment XSD, including `pacs.008.001.08`, `pacs.009.001.08`, `pacs.002.001.10`, `pacs.004.001.09`, `pacs.004.001.10`, `camt.056.001.08`, and `camt.056.001.09`; `scripts/iso_xsd_fixture_verify.py` prevents silent XSD/XML fixture namespace and payload-root drift while recording reviewed missing-schema gaps and profile-advertised message-version gaps | Add remaining official MDR/XSD lifecycle fixtures per profile, make the strict schema-backed XSD/profile preflight pass, and add live-rail gateway adapter coverage |
| Return/cancel lifecycle | Durable outbox helpers exist for `pacs.004`, `camt.029`, `sese.024`, and `sese.025`; known-original return and cancellation transitions have focused Torii coverage plus checked-in `pacs.004` and `camt.056` XML fixtures; full `pacs.004.001.09`, `pacs.004.001.10`, `camt.056.001.08`, and `camt.056.001.09` XSD fixtures now pin live-profile return/cancellation admission where the default rail profiles allow those versions | Add remaining official rail/profile return and cancellation fixture packs |
| Securities crosswalks | Reference snapshots load locally and live securities profile admission validates instrument, active venue MIC, delivering/receiving BIC lookups, configured CSD venue domain, delivering/receiving settlement-account mappings, and securities cash-leg asset mapping before durable lifecycle recording | Keep operator snapshots current and add live-rail adapter coverage around production CSD/account/cash-leg sources |
| Profile catalog | Static defaults plus config overrides; the XSD preflight can now parse the embedded default catalog and report schema-backed coverage for concrete profile-advertised versions | Add fixture coverage against official MDR/XSD releases per profile until `--require-profile-schema-backed-versions` passes |
| Persistence | Digest-bound local JSON state files plus deterministic local audit index; tampered, schema-incomplete, filename-mismatched, symlinked, or oversized records and symlinked record directories are rejected on reload and excluded from regenerated indexes, oversized runtime records are not written as durable files or audit-index entries, symlinked durable-output directories are not followed for record, audit-index, export-index, or digest-addressed notary-preimage writes, the current manifest is exposed through `GET /v1/iso20022/audit/messages`, configured age/count retention compacts records while regenerating the manifest, `audit_export_dir` mirrors digest-bound manifest/notary preimages to an operator-managed external spool, `scripts/iso_audit_notary_adapter.py` verifies and publishes those preimages to configured HTTPS archival/notary endpoints with local receipts, `scripts/iso_operator_receipt_verify.py` gates those receipts, `scripts/iso_operator_canary.py` records one rail/notary/verify summary, `scripts/iso_operator_evidence_verify.py` rejects non-production summaries before archival, and `scripts/iso_production_readiness.py` aggregates XSD/evidence summaries into the final release report | Run provider-specific production canaries against the selected archival/notary vendors and archive evidence summaries that pass the production-readiness gate |

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
