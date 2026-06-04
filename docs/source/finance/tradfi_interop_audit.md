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
  filename, local `messages.index.json` equality, and record-count consistency
  before posting to HTTPS endpoints; local HTTP is rejected unless explicitly
  enabled for tests, endpoint URLs must not contain credentials, params, query
  strings, fragments, or control characters, and every publish attempt writes a
  bounded receipt without persisting bearer-token material.
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
  `pacs.004.001.10`, and `camt.056.001.08` to the offline MDR-derived schema
  set. The live-profile fixture matrix now admits BAH status, return, and
  cancellation reports under the configured version/business-service controls
  for the rails that allow those exact versions.
- Added `fixtures/iso20022/xsd/fixture_manifest.json` plus
  `scripts/iso_xsd_fixture_verify.py`, an offline manifest preflight that binds
  checked-in XSDs to checked-in XML fixtures, verifies schema target namespaces
  and `Document` payload roots, verifies XML fixture namespaces and payload
  roots, enforces canonical lowercase ISO message definition ids and path
  containment for schema/fixture entries, emits digest-bound summaries, and
  makes missing-XSD fixture coverage explicit. All checked-in XSDs now have
  standalone XML fixtures, so `--require-fixture-for-schema` passes; the
  schema-backed strict flag still intentionally fails until the remaining
  official securities/collateral/legacy return XSD packages are checked in.
- Updated OpenAPI and MCP submission surfaces to expose profile selection.
- Added `scripts/iso_rail_gateway_adapter.py`, an operator-side file-drop
  adapter for live rail gateway ingress. Each inbound `*.xml` must have a
  sibling `*.xml.json` sidecar that pins `message_type`, `profile`, and
  `payload_sha256`; the adapter verifies those values before posting to the
  matching Torii ISO endpoint, rejects plaintext HTTP unless explicitly enabled
  for local tests, rejects Torii base URLs with credentials, params, query
  strings, fragments, or control characters, keeps explicit `--message` paths
  inside the declared inbox, requires an explicit profile by default, rejects
  legacy `colr.007` collateral drops unless `--allow-legacy-colr007` is set for
  local diagnostics, and writes bounded local receipts for successful and
  failed submissions.
- Added `scripts/iso_operator_receipt_verify.py`, a read-only canary verifier
  for rail/notary adapter receipts. It recomputes receipt digests, requires
  successful 2xx receipts by default, rejects plaintext HTTP evidence unless
  explicitly enabled for local tests, rejects receipt endpoint URLs with
  credentials, params, query strings, fragments, malformed hosts, or control
  characters, detects leaked authorization/token material, can cross-check
  referenced XML or notary-anchor source files, rejects legacy `colr.007` rail
  source files unless `--allow-legacy-colr007` is set for local diagnostics, and
  emits a digest-bound verifier summary with per-receipt file paths,
  `receipt_sha256` values, and policy flags.
- Added `scripts/iso_operator_canary.py`, a strict JSON-runbook runner that
  executes the rail file-drop adapter, audit notary adapter, and receipt
  verifier as subprocesses. It requires explicit provider/environment labels,
  rejects unknown runbook keys, rejects control characters in runbook strings,
  keeps relative paths inside the runbook directory, rejects endpoint URLs with
  credentials, params, query strings, or fragments, redacts bearer-token file
  arguments in its summary, verifies generated receipts by default, and writes a
  single bounded summary JSON for operator evidence archives. `--plan-only`
  validates runbooks and prints redacted child commands without contacting Torii
  or notary endpoints.
- Added `scripts/iso_operator_evidence_verify.py`, an offline production
  evidence gate for archived canary and trust-bundle summaries. It recomputes
  summary digests, requires successful rail/notary/verify stages by default,
  requires the verify stage to carry digest-bound receipt-verifier JSON with
  rail and notary receipt kinds plus per-receipt digests, rejects plan-only or
  dry-run canaries, plaintext-HTTP overrides, default-profile fallbacks,
  legacy `colr.007` local overrides, unredacted bearer-token paths,
  secret-looking output, smuggled trust-source URLs, missing/malformed/future
  trust-source retrieval timestamps, smuggled child command endpoint URLs,
  local-only child command flags in either `--flag` or `--flag=value` form,
  unsupported child command flags outside the expected rail/notary/receipt
  verifier CLI surfaces, synthetic trust DER, and record-only trust policy
  before an archive is treated as production evidence.
- Added `scripts/iso_production_readiness.py`, an offline release-readiness
  rollup for digest-bound XSD and operator evidence summaries. It requires
  strict schema-backed/fixture-backed XSD proof, production evidence policies,
  full rail/notary/verify canary evidence, digest-bound direct receipt-archive
  verification with per-receipt digests, rail/notary receipt kinds, no legacy
  `colr.007` local overrides, and `require-verified` trust profiles with at
  least one public-key or X.509 trust pin before reporting the ISO corridor
  ready.
- Added checked-in operator canary runbook templates under
  `fixtures/iso20022/operator_canary/` for Swift CBPR+, Fedwire Funds, SEPA SCT
  Inst, and securities CSD profile families.
- Added `scripts/iso_trust_bundle_verify.py`, an offline XMLDSig/XAdES trust
  bundle preflight for operator rail PKI material. It verifies canonical pins,
  digest-bound base64 DER envelopes with lightweight semantic shape checks for
  X.509 certificates, X.509 CRLs, and OCSPResponse wrappers, duplicate and
  contradictory trust/revocation material, required CRL/OCSP material, HTTPS
  provenance without credentials, params, query strings, fragments, malformed
  bracket syntax, control characters, or local/private IP literals,
  timezone-aware non-future retrieval timestamps, unique DER labels per
  material class, and rejects secret-looking fields before emitting Torii
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
  --plan-only

python3 scripts/iso_operator_canary.py \
  --config runbooks/iso/local-provider-canary.json \
  --summary-out run/iso/local-provider-canary.summary.json
```

The JSON runbook is intentionally strict. It must label `provider` and
`environment`, configure at least one of `rail` or `notary`, and enables receipt
verification by default. Relative paths are resolved from the runbook location
and must stay under that directory; use absolute paths for explicitly external
operator directories. Endpoint URLs must not contain embedded credentials,
params, query strings, or fragments. Bearer-token files are runtime-only inputs
passed through to child scripts; the runner never reads token contents and
redacts token-file arguments in the summary. Checked-in templates for Swift
CBPR+, Fedwire Funds, SEPA SCT Inst, and securities CSD live-profile families
live under
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
lowercase nonzero SHA-256 pins, matching DER digests, no duplicate DER within a
material class, unique DER labels within each material class, no trust anchor
that is also revoked, CRL/OCSP material when the corresponding profile flags are
explicitly enabled, explicit CRL/OCSP revocation policy booleans, clean HTTPS
source URLs by default, timezone-aware non-future
`source.retrieved_at` values, and no runtime secret fields. Production bundles
also need DER values that look like the expected material class; the checked-in
templates use synthetic DER envelopes only so CI can validate schema and
emitted-profile wiring, and require `--allow-synthetic-der`.
Synthetic-template validation is summary-only:
`--allow-synthetic-der` cannot be combined with `--emit-profile-json`. Replace
templates with the current rail PKI package before production use and omit that
flag before emitting profile overrides.

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
  --summary-out run/iso/local-provider-evidence.summary.json
```

The gate is offline and fails closed. By default it requires digest-bound,
successful canary summaries with rail, notary, and receipt-verify stages;
requires the receipt-verify stage output to be digest-bound and contain
positive receipt counts, both rail/notary receipt kinds, and per-receipt
`receipt_sha256` entries, with explicit boolean receipt policy fields rather
than omitted defaults; rejects plan-only, dry-run, insecure-HTTP,
default-profile, failed-receipt, legacy `colr.007`, and missing-source-file
evidence; requires trust summaries produced without synthetic DER, record-only
policy, insecure provenance overrides, or malformed/future trust-source
retrieval timestamps, with trust-summary policy booleans present explicitly;
requires archived trust profile overrides to carry explicit CRL/OCSP revocation
policy booleans; and scans archived commands/output for obvious secret leakage.
Plan-only diagnostic archives must still record each planned stage's `dry_run`
boolean. Bearer-token file arguments must be redacted whether represented as
`--bearer-token-file <path>` or `--bearer-token-file=<path>`. The `--allow-*`
flags, including
`--allow-legacy-colr007`, are for local test audits only and should not be
present in production evidence archives.

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
  --summary-out run/iso/production-readiness.summary.json
```

The rollup exits `0` only when every supplied summary proves production posture.
It also requires the evidence summary to include direct receipt archive
verification from `--receipt` or `--receipt-dir`, not only the canary stage's
captured verifier stdout. That archive verification summary must carry its own
`summary_sha256`, production policy flags, and a `receipts[]` list binding each
receipt path to its `receipt_sha256`. XSD strict-mode flags, evidence-level
production policy flags, and nested receipt-summary policy flags must all be
present as booleans; evidence `ok` and canary `plan_only` status fields are
also required booleans, and trust-summary policy flags are enforced by the
evidence gate before the rollup accepts an archive. Omitted flags are malformed
input, not implicit production defaults. It exits `1` with a digest-bound
blocker report when summaries are valid but not production-ready, and exits `2`
for malformed or digest-tampered inputs, including nested receipt-summary
tampering. Evidence summaries or nested receipt summaries that were produced with
`allow_legacy_colr007=true` are production blockers.
`--allow-reviewed-xsd-gaps` and `--allow-canary-stage-receipts-only` exist for
local diagnostic audits of the current checked-in fixture corpus; production
release evidence should omit them and must make the strict XSD and
receipt-archive checks pass.

## Gap Register

| Area | Current state | Target |
| --- | --- | --- |
| Rail connectivity | Local bridge endpoints plus `scripts/iso_rail_gateway_adapter.py`, an operator-side file-drop adapter that verifies sidecar-pinned message type/profile/payload digest before submitting to Torii and writing receipts outside consensus-critical code; `colr.012` is the production collateral-substitution family and legacy `colr.007` requires an explicit local override that receipt/evidence/readiness gates reject for production; `scripts/iso_operator_receipt_verify.py` gates the resulting receipts, `scripts/iso_operator_canary.py` ties the adapter plus verifier into a reproducible provider runbook, `scripts/iso_operator_evidence_verify.py` rejects non-production archived summaries, `scripts/iso_production_readiness.py` aggregates accepted summaries into one release gate, and checked-in Swift/Fedwire/SEPA/CSD templates plan successfully without network access | Run provider-specific live gateway canaries for selected SWIFT/Fedwire/SEPA/CSD operators and archive evidence summaries that pass the production-readiness gate |
| XMLDSig/XAdES | Supported P-256/SHA-256 enveloped subset is verified against profile public-key, leaf-certificate, and linked certificate-chain pins with non-CA XMLDSig leaf certificates carrying critical `keyUsage`/`digitalSignature`, deterministic child issuer distinguished-name binding to parent subject distinguished names, bounded duplicate-free `X509Data` certificate chains, certificate-chain ECDSA-with-SHA256/id-ecPublicKey-secp256r1 enforcement, critical issuer CA `basicConstraints` and `keyUsage`/`keyCertSign`, issuer path-length constraint enforcement, rejection for unknown, malformed, or unsupported parsed critical X.509 extensions, extension/validity checks against verified signed XAdES `SigningTime` or BAH `CreDt`, explicit certificate revocation pins, low-S fixed-width `r || s` or low-S DER ECDSA signature values, one empty or unique same-document `#id` payload Reference URI that strictly encloses the verified signature carrier with an enveloped-signature transform first, at most one final supported C14N transform, one optional XAdES `SignedProperties` Reference with a local `#id`, `QualifyingProperties` target bound to the enclosing `Signature` `Id`, exactly one supported bare `Signature` or direct-child `Sgntr`/`Signature` carrier, ordered direct `Signature`/`SignedInfo` child parsing, prefixed XMLDSig structure bound to the XMLDSig namespace, prefixed XAdES structure bound to the ETSI XAdES v1.3.2 namespace, exact QName opening/closing tag matching in supported XML spans, malformed structural QName rejection, direct `Signature/Object/QualifyingProperties/SignedProperties/SignedSignatureProperties` XAdES parsing, certificate-backed `SigningCertificateV2` ordered duplicate-free chain-prefix digest binding with direct `Cert`/`CertDigest` children only, no unreferenced, wrapped, or duplicate `SignedProperties` elements, parameter-free XMLDSig method/transform elements with exact-one critical methods, exact-name method/digest/Reference policy attribute lookup, exact-one attribute-free `Transforms` wrappers, implemented ordinary attributes only, and ordered direct `Reference` children, singleton required base64 values, unambiguous public-key-or-certificate key material scoped to exactly one structured `KeyInfo`, inherited namespace context for referenced roots, a fail-closed supported canonical XML subset for empty-element expansion, simple attribute normalization, namespace-aware attribute sorting, implicit `xml:` namespace attributes, legal `xmlns:xml` declaration omission, XML-character-reference decoding, no-comments C14N comment omission, and C14N-mode-specific root namespace declarations inherited from the enclosing `Signature`; `scripts/iso_trust_bundle_verify.py` preflights operator trust bundles, `scripts/iso_operator_evidence_verify.py` rejects synthetic/record-only trust summaries for production archives, `scripts/iso_production_readiness.py` rechecks production trust posture in the release rollup, and checked-in Swift/Fedwire/SEPA/CSD templates validate schema and emitted trust overrides | Replace synthetic trust-bundle templates with official profile-specific trust-anchor packages, add complete official canonical XML fixture coverage, add official CRL/OCSP or rail revocation-feed fixtures, and archive evidence summaries that pass the production-readiness gate |
| Follow-up messages | Inbound `pacs.002`, `pacs.004`, `camt.056`, `sese.023`, `sese.024`, `sese.025`, and `colr.012` lifecycle endpoints record durable messages, reject replay evidence, and update known referenced originals only; checked-in payment, securities, and collateral XML fixtures now cover `pacs.002`, `pacs.004`, `camt.056`, `sese.023`, `sese.024`, `sese.025`, and `colr.012` profile/lifecycle handling; the offline MDR/XSD fixture matrix now includes standalone fixtures for every checked-in payment XSD, including `pacs.008.001.08`, `pacs.009.001.08`, `pacs.002.001.10`, `pacs.004.001.10`, and `camt.056.001.08`; `scripts/iso_xsd_fixture_verify.py` prevents silent XSD/XML fixture namespace and payload-root drift while recording reviewed missing-schema gaps | Add remaining official MDR/XSD lifecycle fixtures per profile, make the strict schema-backed XSD preflight pass, and add live-rail gateway adapter coverage |
| Return/cancel lifecycle | Durable outbox helpers exist for `pacs.004`, `camt.029`, `sese.024`, and `sese.025`; known-original return and cancellation transitions have focused Torii coverage plus checked-in `pacs.004` and `camt.056` XML fixtures; full `pacs.004.001.10` and `camt.056.001.08` XSD fixtures now pin live-profile return/cancellation admission where the default rail profiles allow those versions | Add remaining official rail/profile return and cancellation fixture packs |
| Securities crosswalks | Reference snapshots load locally and live securities profile admission validates instrument, active venue MIC, delivering/receiving BIC lookups, configured CSD venue domain, delivering/receiving settlement-account mappings, and securities cash-leg asset mapping before durable lifecycle recording | Keep operator snapshots current and add live-rail adapter coverage around production CSD/account/cash-leg sources |
| Profile catalog | Static defaults plus config overrides | Add fixture coverage against official MDR/XSD releases per profile |
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
