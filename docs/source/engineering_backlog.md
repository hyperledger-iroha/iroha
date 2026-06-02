# Engineering Backlog (Detailed Open Work)

Last updated: 2026-06-02

The public roadmap lives in [`../../roadmap.md`](../../roadmap.md). Completed
history lives in [`../../status.md`](../../status.md). This file should only
track detailed unfinished engineering work.

## FHE/RAM-LFE first-release follow-ups

- Replace the current deterministic exact plaintext-lift BFV-shaped evaluator
  with the full BFV-RNS engine planned for release: bounded RLWE noise, RNS
  modulus chains, real relinearization, packed-slot Galois-key switching, and
  full BFV bootstrapping. The current pass makes Torii/Soracloud consume and
  persist real ciphertext envelopes, evaluates `SelectEqZero` correctly over
  all byte values in the `F_257` RAM-LFE profile, and keeps evaluators
  secret-key free. Soracloud RotateLeft now requires public rotation-key
  refresh material for the outer ciphertext-slot envelope, and Bootstrap
  applies a validated public encrypted-zero refresh key. BFV evaluation-key
  metadata now caps rotation-key bundles and requires canonical bounded
  bootstrap key ids. Those refresh paths are still not a complete BFV-RNS
  bootstrap or packed-polynomial
  Galois-switching circuit.
- Broaden the cross-SDK deterministic BFV-RNS vector corridor: Kotlin, Java,
  Swift, and JavaScript now require `RamLfeOutputOpening` on identifier
  claim/resolve helpers, and a shared Soracloud BFV identifier-envelope fixture
  now covers the baseline encrypted identifier plus three-input Add and
  Multiply operand payloads. The same fixture now pins Rust executor output
  lengths, SHA-256 digests, and plaintext slots for Soracloud Add, Multiply,
  RotateLeft, and Bootstrap operation vectors, as well as deterministic public
  key/public-parameter byte lengths and SHA-256 digests, evaluation-key bundle
  byte length, SHA-256 digest, domain-separated digest, decomposition metadata,
  relinearization entry count, per-relinearization-entry `b`/`a`
  coefficient-vector digests, rotation key count, bootstrap key id,
  rotation/bootstrap encrypted-zero refresh digests, and refresh `c0`/`c1`
  coefficient-vector digests. JavaScript, Swift, Kotlin/JVM, and Java Android
  now validate those component-vector fields from the shared fixture, and the
  JavaScript lane also carries adversarial fixture mutations for missing,
  duplicate, zeroed, and count-drifted component metadata. A shared
  signed/proof-attestation identifier receipt fixture now pins canonical payload
  bytes, Iroha prehash, resolver signature, signed/proof attestation bytes, and
  adversarial receipt/policy mutations across the Rust data model, JavaScript,
  Swift, Kotlin/JVM, Java Android, and Torii runtime claim-receipt signing path.
  The Soracloud FHE governance fixtures now bind the canonical parameter set,
  execution policy, governance bundle, and job spec to the registered
  `bfv-default` RAM-LFE BFV runtime descriptor and reject descriptor drift in
  core admission. The execution policy now also carries the canonical
  evaluation-key bundle digest from the shared operation fixture, and
  `RunSoracloudFheJob` rejects structurally valid but ungoverned key material
  before output state is emitted. Shared release vectors still need to cover
  the full BFV-RNS modulus-chain, packed Galois-switching, and bootstrapping
  key bundle.
- Broaden validation from the green focused crypto/data-model/core/Torii/daemon
  checks into the next full workspace and SDK corridor. Focused adversarial
  tests now cover malformed/truncated ciphertext envelopes, hidden-program
  shape/overflow rejection, replayed/tampered/future/expired/wrong-verifier
  openings, receipt-signing/backend mismatch refusal, adversarial BFV public
  parameters and evaluation-key metadata, execution-policy evaluation-key
  digest mismatches, unregistered BFV parameter sets,
  impossible decrypted identifier envelopes, FHE governance lifecycle/linkage
  abuse, operation-shape and budget-smuggling jobs, encrypted-only Torii DTO
  rejection, duplicate JSON encrypted/opening-field and nested shadow-field
  rejection before DTO decoding,
  full receipt/opening security-binding mutation checks, proof-only receipt
  attestations passed to Rust/JavaScript/JVM SDK signature verifiers,
  wrong resolver keys, mismatched receipt policy ids, validly re-signed but
  execution-mismatched output openings on `ClaimIdentifier`, missing/malformed
  Soracloud evaluation keys, empty/malformed ciphertext slots, malformed
  relinearization keys, structurally valid wrong BFV key-bundle component
  material, malformed SDK ciphertext hex, plaintext-only policy misuse,
  slot-count/digest mismatches, shared signed receipt canonical payload
  drift, shared signed/proof attestation canonical byte drift, malformed
  signatures, wrong resolver keys, wrong policy ids, tampered output ciphertext
  hashes, proof-only attestations, ZK-ACE public-input version drift, and
  ZK-ACE prepared authorization proofs rebound to a different transfer digest,
  chain id, receiver, amount, or policy hash across the Rust data-model, JS,
  Swift, Kotlin/JVM, Java Android, and Torii runtime fixture corridor, plus
  core ZK-ACE rotated/revoked identity state, unsupported action classes,
  transaction digest/account substitution, and mutated ZK-ACE/STARK public
  inputs;
  RAM-LFE proof-verifier metadata now rejects noncanonical backend/circuit
  identifiers, zero schema hashes, empty/all-zero verifier keys, and oversized
  verifier keys before proof-carrying programmed policies are admitted;
  programmed BFV public-parameter admission now rejects zero hidden-program
  digests and relinearization-only violations where unused rotation/bootstrap
  refresh keys are smuggled into identifier-program metadata;
  BFV evaluation-key metadata now rejects noncanonical bootstrap key ids and
  oversized rotation-key bundles before key-bundle digests are admitted;
  generic RAM-LFE and identifier receipt proof verifiers now have focused
  pre-parse regressions for public-input schema drift and non-zero mismatched
  verifier-key hashes;
  secp256k1 recoverable prehash signing now normalizes low-S output and the
  public-key recovery primitive rejects high-S malleable encodings before
  deriving EVM addresses;
  Ed25519 uncached batch verification now rejects noncanonical or small-order
  signature `R` encodings before entering the dalek batch backend;
  X25519 public-key decoders for hybrid KEM keys, hybrid ephemeral ciphertext
  keys, and the standalone key-exchange surface now reject low-order encodings
  before ECDH while retaining all-zero shared-secret fallback checks;
  SoraNet NK2/NK3 handshake parsers now reject low-order Noise static and
  ephemeral public keys in decoded client and relay frames, reject malformed
  Dilithium3/Ed25519 handshake signature field lengths, require 1024-byte
  zero-padded frames, and reject selected KEM/signature ids that are absent
  from either peer's advertised capability TLVs; unsupported KEM ids fail at
  the KEM profile gate before downgrade telemetry is built;
  SoraNet signed-ticket decode and direct verification now reject ML-DSA-44
  signature vectors whose length disagrees with the suite metadata before
  accepting tokens or entering backend verification;
  SoraNet revocation-store reload now rejects duplicate persisted
  fingerprints, rejects overflowing expiry timestamps, and bounds loaded active
  records to the configured capacity;
  SoraNet guard-directory snapshot decode now rejects duplicate or
  key-mismatched issuer fingerprints and enforces ML-DSA-65 issuer public-key
  length/phase requirements before snapshots are admitted;
  SoraNet admission-token replay-store reload now rejects duplicate persisted
  token IDs and overflowing expiry timestamps, and admission-token verification
  preflights ML-DSA issuer public-key and detached-signature lengths before
  backend verification or replay-store mutation; SoraNet SRCv2 bundle
  verification rejects weak Ed25519 verifier keys and preflights ML-DSA-65
  issuer public-key and detached-signature lengths before backend verification;
  local SRCv2 issuance reuses certificate-payload admission and ML-DSA-65
  issuer secret-key length preflight before signing bundles; Phase 2 SRCv2
  rollout accepts Ed25519-only relay certificates while Phase 3 remains the
  dual-signature gate;
  SoraNet SRCv2 certificate decode now rejects unknown ML-KEM suite ids and
  key-material length drift for ML-DSA-65 identity keys and advertised ML-KEM
  relay public keys, rejects malformed/noncanonical/weak Ed25519 identity
  public keys, rejects ML-DSA-65 detached signature length drift, and its
  canonical CBOR parser rejects trailing payload/bundle bytes plus non-shortest
  integer/length encodings and duplicate nested
  bundle/signature/endpoint/KEM-policy fields; guard-directory relay entries
  now parse as SRCv2 bundles and must bind to a known snapshot issuer, the
  snapshot directory hash, and a unique relay ID, with relay certificate
  signatures verified against embedded issuer keys under the snapshot
  validation phase; zero-length or inverted snapshot validity windows now fail
  closed, and relay certificate validity must cover the full snapshot window
  without being published after the snapshot; SRCv2 role/capability bitmask decode rejects unsupported
  bits instead of masking them away and validity windows fail closed when they
  are inverted or published after expiry; KEM rotation policies reject static
  fallback/rotation/grace metadata, staged policies without fallbacks, rolling
  policies without nonzero cadence, and preferred/fallback suite equality;
  handshake-suite preference lists and endpoint URL lists must be non-empty and
  duplicate-free, and endpoint URL strings reject empty,
  whitespace-bearing, or control-character values; endpoint tags, when present,
  reject empty, whitespace-bearing, control-character, or duplicate values;
  remaining breadth should emphasize full cross-SDK RNS vectors and broader
  release validation.

## Kotodama first-release follow-ups

- Completed 2026-06-01: static compiler-derived access descriptors now cover
  the formerly opaque peer, subscription, VRF epoch seed, AXT, and Soracloud
  host helper syscalls. Dynamic, malformed, and syscall/operation-mismatched
  payloads stay in the incomplete-hint path and are rejected by production
  compilation instead of being represented with wildcard fallbacks.

## Nexus independent lane consensus follow-ups

- Replace the current global proposal-path lane lookahead with the full
  per-lane proposal/vote scheduler so lane blocks are proposed, executed, and
  QC-sealed by their own lane committees instead of being emitted as relay
  metadata from the global block path.
- Wire lane-local DA/RBC payload ownership into that scheduler and persist lane
  block artifacts independently from global block sealing.
- Add a multi-peer integration corridor proving two active lanes can advance at
  different heights, produce lane-domain QCs, upgrade FastPQ relay proofs, and
  merge without waiting for an idle configured lane. Broaden the unit-level
  committed-record hydration coverage into restart/replay coverage for
  persisted verified relay records.

## Cross-dataspace AMX follow-ups

- Add a multi-peer integration corridor that proves native AMX receipts emitted
  by the universal coordinator survive block relay, Sumeragi status export, and
  downstream audit consumption.
- Extend SDK/OpenAPI convenience models for lane settlement commitments so
  native AMX receipt legs are first-class in client responses instead of only
  available through generic commitment decoding.
- If future coordinator execution supports partial prepare failure inside a
  batch, extend `NativeAmxReceipt` with explicit abort evidence. Finalized
  receipts currently represent only successfully committed native AMX batches.

## Transaction pipeline follow-ups

- Broaden fee/gas/Nexus detached postprocessing beyond the current simple
  transparent single-transfer case. Remaining work includes deterministic
  receipt/effect representation for multi-instruction and multi-asset deltas,
  plus data-trigger-aware fee event ordering. Those shapes intentionally remain
  visible as `fee_postprocessing` detached fallback reasons in Sumeragi status
  and pipeline telemetry.
- Broaden validation from the focused scheduler, dynamic IVM access, telemetry,
  fee-enabled transfer, query-continuation, and receipt-hash tests to the next
  long `cargo test --workspace` corridor once the repository-wide dirty
  worktree settles.

## Torii query API follow-ups

- Completed 2026-06-01: audited endpoint-specific OpenAPI schemas and SDK
  convenience parsers for app endpoints that expose concrete response models.
  Account, domain, account-asset, asset-definition, NFT, RWA, asset-holder, and
  repo-agreement list/query responses now share concrete page schemas that
  document required `has_more`, required `count_mode`, and optional `total`;
  JavaScript and Python convenience parsers preserve bounded page metadata and
  reject malformed count flags before treating a response as valid.
- Completed 2026-06-01: added sustained Torii query-load profiles to
  `torii_hot_paths` for signed iterable `/query` in stored-cursor bounded mode,
  primary account-alias projections, account-asset predicates, asset-holder
  scans, committed-history contract-activity predicates, and generic aggregate
  queries under concurrent in-process HTTP clients. The signed profile walks
  deep continuation chains over the Arc-backed snapshot replay path, the
  contract-activity profile builds real committed transactions with contract
  metadata, and `query_load_profiles` rejects malformed or adversarial
  benchmark shapes before fixture construction.
- Completed 2026-06-01: added a localhost socket transport group for the same
  sustained Torii query profiles. The socket group binds ephemeral Axum
  listeners and drives them with pooled `reqwest` clients so handler-only
  measurements can be compared with real HTTP transport and body IO overhead.
- Run the full signed/app socket profile suite under production-like datasets
  and longer measurement windows to decide whether the existing account-asset,
  asset-holder, and contract-activity predicates need additional indexes or
  materialized views.

## SoraFS paid pin validation follow-ups

- Rerun the full SoraFS data-model, core pin-registry, Torii storage-pin, and
  gateway policy suites under the next long validation budget. Focused coverage
  is now green for the paid-pin adversarial cases; remaining breadth should
  include historical fee receipt acceptance after governance pricing changes,
  manifest envelope validation, admission fail-closed, streaming CAR range
  coverage, and SDK validation once Java is available.

## ZK audit validation follow-ups

- Fold the now-green focused ZK cleanup and adversarial negative corridor into
  the next long `cargo test --workspace` / CI validation budget.

## TradFi ISO 20022 interop follow-ups

- Completed 2026-06-01: added inbound lifecycle endpoints for `pacs.002`,
  `pacs.004`, `camt.056`, `sese.023`, `sese.024`, and `sese.025`, with OpenAPI
  and MCP submission surfaces. The bridge records each lifecycle message in the
  durable ISO record model, rejects duplicate payload, business-message-id, and
  UETR replays, applies `pacs.002`/`pacs.004`/`camt.056` and
  `sese.024`/`sese.025` updates only when the referenced durable record is
  known, and keeps `sese.023` as a recorded settlement instruction until all
  account, instrument, venue, CSD, and
  cash-leg crosswalks are configured for ledger instruction mapping.
- Completed 2026-06-01: added durable-record outbox helpers for `pacs.004`,
  `camt.029`, `sese.024`, and `sese.025`. Payment returns require recorded
  settlement amount/currency from the original payment message; securities
  confirmations require captured `sese.023` amount, currency, quantity, movement,
  payment, and execution-plan fields rather than fabricating missing data.
- Completed 2026-06-01: added a fail-closed Torii verification path for
  `require-verified` embedded-signature profiles. The bridge now accepts the
  supported P-256/SHA-256 enveloped XMLDSig/XAdES subset only after payload
  digest and signature verification, rejects tampered digests/signatures and
  unsupported algorithms, and keeps live `reject-unsupported` profiles rejecting
  embedded signature blocks.
- Completed 2026-06-01: added profile-specific XMLDSig trust pins for
  `require-verified` profiles. Torii now rejects otherwise valid signed
  payloads unless the verified raw public key or DER certificate SHA-256 digest
  matches the selected rail profile, rejects non-canonical/all-zero configured
  pins at startup, and covers the supported C14N 1.0, C14N 1.1, and exclusive
  C14N algorithm identifiers with deterministic fixtures.
- Completed 2026-06-01: added XMLDSig/XAdES certificate-chain verification for
  `KeyInfo/X509Data`. Torii now accepts at most eight unique DER certificates,
  derives the signing key from the leaf
  certificate, verifies each supplied leaf-to-issuer chain link by binding the
  child issuer distinguished name to the parent subject distinguished name and
  checking the child signature before exposing issuer/root DER SHA-256 digests
  to the selected profile, requires leaf critical `keyUsage` carrying
  `digitalSignature` while rejecting CA leaf certificates, requires issuer
  critical CA `basicConstraints` plus critical `keyUsage` carrying
  `keyCertSign`, requires every supplied certificate to use ECDSA-with-SHA256
  over id-ecPublicKey secp256r1 with uncompressed P-256 SEC1 subject public-key
  bytes, enforces issuer `pathLenConstraint` values for subordinate CA chains,
  rejects unknown, malformed, or unsupported parsed critical X.509 extensions
  on every supplied certificate, checks leaf and issuer certificate validity
  against deterministic verified signed `SigningTime` or BAH `CreDt`, and
  covers the pinned-issuer accept/reject corridor with generated P-256 fixtures.
- Completed 2026-06-02: added a deterministic supported XML canonicalization
  subset for XMLDSig verification. Torii now canonicalizes `SignedInfo` and the
  referenced enveloped payload before hashing or signature verification. The
  supported Reference URI scope is a single empty URI or a unique same-document
  `#id` target using exact `Id`, `ID`, `id`, or `xml:id` attributes; remote,
  empty-fragment, duplicate-ID, namespace-qualified non-`xml` ID attributes, and
  same-document payload targets that do not strictly enclose the verified signature carrier
  fail closed, while selected same-document targets carry ancestor namespace
  declarations into root canonicalization. Each supported payload Reference must
  declare an enveloped-signature transform first, may add at most one final
  supported C14N transform that controls digest canonicalization, and must use a
  SHA-256 digest method; missing, reordered, extra, or unsupported transforms
  fail closed. The verifier also accepts one optional XAdES `SignedProperties`
  Reference with the XAdES `SignedProperties` Type URI, a local `#id` target, one
  supported C14N transform, and a SHA-256 digest; its enclosing
  `QualifyingProperties` target must bind to the enclosing `Signature` `Id`, and
  certificate-backed XAdES signatures must present a non-empty, duplicate-free
  ordered prefix of the verified XMLDSig certificate-chain SHA-256 digests,
  starting with the leaf certificate. The supported signed-property subset
  requires direct
  `Signature/Object/QualifyingProperties/SignedProperties/SignedSignatureProperties`
  structure; `QualifyingProperties` accepts only `Target`,
  `SigningCertificateV2` accepts only attribute-free direct `Cert` children with
  attribute-free direct `CertDigest` children, a `DigestMethod` carrying only
  `Algorithm`, and text-only digest values. Signed `SigningTime` is a singleton
  attribute-free text leaf. Any `SignedProperties` element under the signature
  must be the verified referenced direct target; unreferenced, wrapped, or
  duplicate `SignedProperties` elements and unrelated additional References fail
  closed.
  Supported XMLDSig method and transform elements are parameter-free: `CanonicalizationMethod`,
  `SignatureMethod`, `DigestMethod`, payload Reference transforms, and
  `SignedProperties` transforms reject non-whitespace child content such as
  `InclusiveNamespaces`, XPath, HMAC, or digest parameters. Critical XMLDSig
  method elements must appear exactly once, Reference transforms must be
  enclosed in exactly one attribute-free `Transforms` wrapper, and only
  implemented ordinary attributes are accepted (`Algorithm`, payload Reference
  `URI`, and XAdES Reference `URI`/`Type`). Extra direct children under
  `Reference` or `Transforms` fail closed, and supported References must keep
  direct children ordered as `Transforms`, `DigestMethod`, then `DigestValue`.
  Top-level `Signature` and
  `SignedInfo` parsing now accepts only implemented direct children in supported
  XMLDSig order, so reordered or wrapped `SignedInfo`/method nodes, unsupported
  direct children, and duplicate singleton signature nodes fail closed. The
  payload may contain exactly one supported signature carrier: either a bare
  XMLDSig `Signature` or an ISO `Sgntr` wrapper with exactly one direct XMLDSig
  `Signature` child. Any additional `Signature`/`Sgntr` element outside the
  verified carrier fails closed. Required XMLDSig base64 fields
  such as `SignatureValue`, per-Reference `DigestValue`, and XAdES
  `CertDigest` values reject duplicates and must be attribute-free text leaves
  without nested markup or comments; `PublicKey` must be singular, and
  `PublicKey`/`X509Certificate` credential leaves follow the same no-markup
  rule. Public-key material must not be mixed with `X509Certificate` material
  in the same `KeyInfo`. Key material must be scoped to exactly one `KeyInfo`
  using either `KeyValue/ECKeyValue` with the P-256 `NamedCurve` URI whose
  `PublicKey` bytes parse as an uncompressed P-256 SEC1 point, or one bounded
  duplicate-free `X509Data` certificate-chain wrapper; those wrappers accept
  only implemented direct children, and unsupported children, unsupported
  ordinary attributes, non-whitespace wrapper text, duplicates, or out-of-scope
  `PublicKey`/`X509Certificate` elements fail closed. The canonical subset
  covers empty-element expansion, attribute quote normalization, namespace
  declarations, unprefixed attributes, declared prefixed attributes, and implicit
  `xml:` attributes while accepting and omitting the fixed legal `xmlns:xml`
  declaration. It
  also decodes predefined and numeric XML character references before
  re-emitting canonical text/attribute bytes. It applies root namespace
  declarations inherited from an enclosing XMLDSig `Signature` element according
  to the declared C14N mode: inclusive C14N carries all inherited root namespace
  declarations, while exclusive C14N carries only visibly used inherited root
  namespace declarations. No-comments C14N now omits
  valid XML comments from `SignedInfo` and referenced payload bytes while
  rejecting malformed comments. The verifier still rejects processing
  instructions, CDATA/CDEnd tokens, uppercase `#X` numeric character
  references, DTD/general/custom entity expansion, carriage returns, duplicate
  attributes, unbound prefixed attributes, explicit reserved namespace
  rebindings, malformed structural QNames such as double-colon local-name
  matches, inherited namespace context beyond root declarations, raw attribute
  whitespace rewrites, and malformed tag structure.
- Completed 2026-06-02: broadened XMLDSig ECDSA `SignatureValue`
  interoperability. Torii now accepts the fixed-width P-256 `r || s` signature
  encoding used by XMLDSig profiles while retaining DER fixture compatibility,
  requires canonical low-S for both encodings to remove ECDSA malleability, and
  the require-verified suite covers accepted low-S plus rejected high-S
  signatures.
- Completed 2026-06-02: hardened XMLDSig namespace binding for the supported
  signed ISO subset. Prefixed XMLDSig structural elements must now resolve to
  the XMLDSig namespace in their inherited scope across `Signature`,
  `SignedInfo`, `Reference`/`Transforms`/`Transform`/`DigestMethod`/
  `DigestValue`, and public-key or X.509 `KeyInfo` material, with regressions
  covering a correctly signed payload that binds `ds` to a non-XMLDSig URI.
  Unprefixed XMLDSig structural elements remain accepted for legacy fixtures
  only when they do not carry an explicit conflicting default namespace.
- Completed 2026-06-02: tightened supported XML element span matching so a
  selected opening tag must close with the exact same qualified name. This keeps
  local-name discovery for prefixed XMLDSig fixtures while rejecting malformed
  mismatched-prefix close tags before structure or cryptographic verification
  continues.
- Completed 2026-06-02: tightened XMLDSig attribute value extraction to exact
  XML attribute names. Namespace-qualified spoof attributes such as
  `ds:Algorithm` or `ds:URI` no longer have a local-name fallback in the
  accessor and remain rejected before method, transform, digest, or Reference
  policy is evaluated.
- Completed 2026-06-02: hardened XAdES namespace binding for the supported
  signed-property subset. Prefixed XAdES structural elements now must resolve to
  the ETSI XAdES v1.3.2 namespace (`http://uri.etsi.org/01903/v1.3.2#`) across
  `QualifyingProperties`, `SignedProperties`, `SignedSignatureProperties`,
  `SigningTime`, `SigningCertificateV2`, `Cert`, and `CertDigest`; referenced
  `SignedProperties` targets carry inherited namespace scope into verification,
  and wrong-namespace XAdES payloads fail closed even when re-signed. Unprefixed
  XAdES structural elements now also reject explicit conflicting default
  namespaces.
- Completed 2026-06-02: added profile-level XMLDSig certificate revocation
  pins. Operators can configure `revoked_certificate_sha256` alongside the
  trust pins; Torii validates the SHA-256 deny list at startup and rejects an
  otherwise trusted XMLDSig chain when any verified leaf/issuer DER digest is
  explicitly revoked.
- Completed 2026-06-02: tightened ISO XMLDSig X.509 production admission so
  Torii config and shared profile JSON SHA-256 trust/revocation pins must
  already be canonical lowercase hex, `x509_trust_anchor_sha256_pins` and
  legacy certificate pins require a linked issuer certificate beyond the leaf,
  and CRL/OCSP freshness plus delegated OCSP responder certificate validity are
  evaluated at verified XAdES `SigningTime` or BAH `CreDt` rather than local
  wall clock.
- Completed 2026-06-02: documented the XMLDSig trust-anchor rotation pattern
  for operators: overlap current and next certificate pins during upstream
  cutover, remove the retired pin after cutover, and use
  `revoked_certificate_sha256` only for compromised leaf/anchor digests that
  must override otherwise valid trust pins.
- Completed 2026-06-02: tightened the ISO OCSP DER parser used by
  `require-verified` XMLDSig/XAdES revocation checks. Torii now rejects
  non-shortest long-form DER lengths and non-minimal positive integer encodings
  before OCSP status, responder, or signature validation.
- Completed 2026-06-02: tightened the supported ISO OCSP subset so
  `ResponseData` and `SingleResponse` extensions fail closed instead of being
  ignored by the local parser. Full OCSP extension-policy processing remains
  outside the first-release subset.
- Completed 2026-06-02: extended ISO bridge idempotency to business message
  identifiers. Torii now indexes trimmed `BizMsgIdr`/BAH business-message IDs
  alongside payload hashes and normalized UETRs, rejects replay by business
  message id across distinct durable message records, and preserves the existing
  conflict guard when a rejected message is retried with another record's
  business message id.
- Completed 2026-06-02: tightened reference snapshot checksum coverage for
  profile validation. Torii now has focused coverage proving inbound admission
  metadata records the exact `ReferenceDataSnapshots::snapshot_id()` checksum
  after a BIC/LEI snapshot is loaded, and that the loaded-snapshot checksum
  differs from the all-missing default snapshot.
- Completed 2026-06-02: broadened Torii ISO profile/lifecycle transition
  coverage. Profile admission now returns `UnknownMessageType` when the selected
  rail profile has no inbound message profile for the submitted endpoint family,
  rejects BAH `MsgDefIdr` values outside the selected profile's version set, and
  covers known-original `pacs.004` return plus `camt.056` cancellation paths down
  to durable original-message and lifecycle-message status fields.
- Broaden XMLDSig/XAdES fixture coverage beyond internal P-256 key and
  generated certificate-chain material, including complete canonical XML
  coverage for broader signed ISO envelopes, official
  rail/profile-specific trust-anchor packages, official CRL/OCSP or rail
  revocation-feed fixtures.
- Add official MDR/XSD fixture coverage per profile.
- Completed 2026-06-01: tightened the deterministic XMLDSig/XAdES subset so
  `require-verified` profiles only accept the C14N 1.0 + single enveloped
  transform shape that the verifier actually checks. C14N 1.1, exclusive C14N,
  extra transforms, and duplicate `Sgntr` blocks now fail closed.
- Completed 2026-06-01: added profile-level
  `signature_public_key_sha256_pins` for `require-verified` XMLDSig/XAdES
  profiles. The verifier now fails closed without configured pins, accepts raw
  XMLDSig public keys and X.509 certificate subject public keys only when their
  SHA-256 pin matches the profile, rejects malformed/all-zero pins, and rejects
  ambiguous or duplicate key material.
- Completed 2026-06-01: added profile-level
  `x509_trust_anchor_sha256_pins` for X.509 XMLDSig key-info chains in the
  supported P-256/SHA-256 subset. The verifier now validates leaf-to-anchor
  issuer links, ECDSA certificate signatures, certificate validity windows,
  CA/keyCertSign trust anchors, duplicate certificates, non-CA anchors, missing
  anchors, issuer mismatches, and trust-anchor DER SHA-256 pins before using an
  X.509 leaf key that is not directly pinned.
- Completed 2026-06-01: added profile-level
  `x509_required_certificate_policy_oids` for rail-specific X.509 XMLDSig
  signer policy gates. X.509 leaf certificates must now carry every configured
  certificate-policy OID before either direct leaf-key pins or validated
  trust-anchor chains can authorize the XMLDSig key; malformed configured OIDs,
  missing policy extensions, and wrong policy OIDs fail closed.
- Completed 2026-06-01: added profile-level CRL revocation enforcement for
  X.509 XMLDSig key-info chains. Profiles can require a fresh verified CRL via
  `x509_require_crl_revocation_check` and can supply pinned rail CRL material
  through `x509_crl_der_base64`; embedded `X509CRL` material is accepted only
  on the X.509 path. The verifier checks CRL DER parsing, issuer matching,
  issuer `cRLSign`, ECDSA/SHA-256 CRL signatures, freshness windows, duplicate
  CRL rejection, missing required CRLs, wrong issuers, expired CRLs, and revoked
  signer serials before using the X.509 leaf key.
- Completed 2026-06-01: added fail-closed X.509 name-constraint processing for
  trust-anchor-authorized XMLDSig key-info chains. The verifier now enforces
  permitted and excluded subtrees from constrained issuer certificates across
  subordinate signer certificates before using the leaf key, with local support
  for DNS, RFC822, URI-host, IP subnet, and directory-name forms and closed
  rejection for unsupported or invalid general names.
- Completed 2026-06-01: added profile-level OCSP revocation enforcement for
  X.509 XMLDSig key-info chains. Profiles can require fresh OCSP coverage via
  `x509_require_ocsp_revocation_check` and can supply pinned rail response
  material through `x509_ocsp_response_der_base64`; embedded `OCSPResponse` and
  `EncapsulatedOCSPValue` material is accepted only on the X.509 path. The
  verifier parses BasicOCSPResponse DER, binds SHA-256 CertID values to the
  signer and issuer, verifies issuer-signed and delegated ECDSA/SHA-256
  responders, enforces OCSPSigning EKU/digitalSignature key usage for
  delegated responders, checks producedAt/thisUpdate/nextUpdate freshness, and
  rejects missing, revoked, unknown, duplicate, stale, malformed, or unauthored
  responses before using the X.509 leaf key.
- Completed 2026-06-01: added X.509 path-length constraint enforcement for
  trust-anchor-authorized XMLDSig key-info chains. The verifier now evaluates
  BasicConstraints `pathLenConstraint` values across intermediate CAs and
  rejects a chain when a constrained root or intermediate authorizes more
  subordinate CA certificates than its policy allows.
- Completed 2026-06-01: required X.509 XMLDSig signer certificates to be
  end-entity certificates. The verifier now rejects signer leaves whose
  BasicConstraints extension is missing or CA:true before either direct public
  key pins or trust-anchor chains can authorize the key, with adversarial
  coverage for CA-capable signer certificates accepted by neither path.
- Completed 2026-06-01: added fail-closed unknown-critical X.509 extension
  handling for XMLDSig signer material. Critical extensions that the parser
  cannot decode or recognize now reject direct-pinned leaves, trust-anchor
  chains, and delegated OCSP responder certificates before any public key is
  accepted.
- Completed 2026-06-01: made X.509 signer certificate validity windows
  mandatory before direct public-key pins can authorize a key. Expired signer
  leaves now fail before direct-pin acceptance as well as on trust-anchor
  chains, with coverage for both paths.
- Completed 2026-06-01: added X.509 signer Extended Key Usage purpose binding
  for XMLDSig signer material. Signer leaves without EKU remain acceptable, but
  EKU-constrained leaves must allow `codeSigning`, `anyExtendedKeyUsage`, or
  the document-signing OID before either direct public-key pins or trust-anchor
  chains can authorize the XMLDSig key; incompatible server-auth-only signer
  leaves fail closed on both paths.
- Completed 2026-06-01: added X.509 Authority Key Identifier / Subject Key
  Identifier binding for trust-anchor XMLDSig chains. When a subordinate
  certificate presents an AKI key identifier and the issuer presents an SKI, the
  identifiers must match before the trust-anchor path can authorize the leaf
  key; issuer-name/signature-valid chains with mismatched key identifiers fail
  closed.
- Completed 2026-06-02: added conservative required certificate-policy path
  continuity for X.509 trust-anchor XMLDSig chains. When a profile requires
  certificate policy OIDs, every intermediate CA below the pinned terminal
  anchor must carry all required OIDs or `anyPolicy`; generated chain tests
  cover matching, `anyPolicy`, missing, and unrelated intermediate policies.
- Completed 2026-06-02: fail closed on policy mappings, policy constraints,
  and inhibit-any-policy extensions in XMLDSig X.509 material until full RFC
  5280 policy-tree processing is implemented.
- Remaining ISO signature work is optional full RFC 5280 policy-tree processing
  if production profiles need to accept policy mappings, policy constraints, or
  inhibit-any-policy instead of rejecting those extensions in the supported
  subset.
- Completed 2026-06-01: tightened ISO idempotency so replayed Business
  Application Header `BizMsgIdr` values are rejected across different durable
  message identifiers, including after durable-store reload. Live-profile
  validation now also has regression coverage proving recorded metadata carries
  the exact reference-data snapshot checksum and that checksum changes when the
  loaded reference snapshot provenance changes.
- Completed 2026-06-01: broadened live-profile mismatch and lifecycle
  transition coverage. Swift CBPR+ validation now has negative tests for
  unsupported message-definition versions and business services, while
  `pacs.002`, `pacs.004`, `camt.056`, and `sese.025` lifecycle updates fail
  closed when the referenced durable record belongs to the wrong ISO family.
- Completed 2026-06-01: added XSD document-root admission for real ISO XML
  parsing. Each supported XML family now has a canonical `Document` child-root
  gate, and real XML with a missing or mismatched family root fails before
  field-level validation can materialize a message.
- Completed 2026-06-01: added live rail XSD/profile fixture coverage for the
  embedded Swift CBPR+, Fedwire Funds, SEPA SCT Inst, and securities CSD
  profiles. The fixture matrix now validates accepted `pacs.008`/`pacs.009`
  samples against required reference data, business services, message
  definition versions, reference snapshot metadata, and minor-unit policy, with
  adversarial wrong-service, wrong-version, and fractional-amount drift cases.
- Completed 2026-06-01: added offline Standards Editor generated MDR/XSD
  fixtures for `pacs.008.001.08` and `pacs.009.001.08` and bound them to the
  live rail profile matrix. Swift CBPR+, Fedwire Funds, SEPA SCT Inst, and the
  securities CSD profile now each validate at least one live profile payload
  whose namespace and `Document` child root are asserted against the checked-in
  XSD, with a root-drift negative case proving mismatched MDR roots fail before
  profile admission.
- Completed 2026-06-01: kept backward-compatible `trusted_public_key_sha256`
  and `trusted_certificate_sha256` profile aliases while normalizing them into
  the stricter `signature_public_key_sha256_pins` and
  `x509_trust_anchor_sha256_pins` verifier inputs.
- Broaden XMLDSig/XAdES fixture coverage beyond pinned P-256 key/certificate
  material, including full certificate-chain fixtures and official
  rail/profile-specific trust-anchor packages.
- Add official MDR/XSD fixture coverage per profile and broaden Torii tests for
  profile mismatch, cancellation/return transitions, reference snapshot
  checksum expectations, and replay by business message id/UETR.

## Soracles follow-ups

- Add the off-chain/runtime leader scheduler and pacemaker automation for
  provider fetches and manual aggregate replacement. The MVP keeps deterministic
  committee/leader derivation and quorum checks, but does not yet schedule
  leaders at runtime.
- Add provider rating weights and governance-driven reputation adjustments once
  enough provider stats are available from live feed history. Current
  aggregation remains equal-weight median/percentile with deterministic
  provider counters.

## FASTPQ GPU acceleration follow-ups

- Evaluate whether to promote the low-level Poseidon fused column+parent kernel
  from parity-only coverage to the production hot path. CUDA and Metal parity
  evidence for the current high-level column + Merkle-pair GPU path is now
  recorded in `status.md`; acceptance for a low-level hot-path promotion still
  requires a fresh Izanami gate/profile showing a real throughput improvement
  over that high-level path, with scalar CPU remaining the authoritative
  fallback for every mismatch or dispatch error. No CUDA-specific FASTPQ proof,
  parity, benchmark, or release-comparison task remains open here.

## Sumeragi vNext consensus replacement

- Optimize from the cap `1096` / pipeline `250ms` 20k liveness baseline toward
  higher applied throughput while preserving the hard 2-3s consensus cadence
  gate. The current confirmed 300s stable point is
  `dist/izanami-liveness-matrix-20k-cap1096-p250-pi5-soak-300s-20260511-074409`:
  scan multiplier `32`, collectors/redundant-send `3/3`, backup RBC on, all
  `6,000,000` submissions accepted, strict height `126`, zero view changes,
  runner p95 `2523ms`, parsed peer p95 `2.899s`, max peer gap `3.833s`, full
  detached merge (`1096/1096`, fallback `0`), and `453.13` committed TPS.
  Higher rows are rejected for now: cap `1100`/`250ms` with `3/3` collectors
  failed the parsed peer p95 gate at `3.071s`, cap `1100`/`250ms` with `4/4`
  collectors still failed at `3.022s` with lower committed throughput, cap
  `1104`/`250ms` failed the parsed peer p95 gate at `3.054s` under the finer 5s
  progress monitor, cap `1120`/`250ms` failed both runner and parsed p95 gates,
  cap `1120`/`300ms` was only a runner-gate near miss, and the older cap
  `1312` 120s pass failed the 300s soak. Next, target DA/precommit and
  peer-gap tail reduction before trying to raise cap again. Accept a result
  only if the runner gate, parsed peer p95 gate, zero-view-change requirement,
  and detached-merge counters remain green. Keep backup-on as the default
  recovery posture and use backup-off only as an explicit experiment row. The
  4,096, 8,192, and 16,384 cap experiments already proved that much larger
  blocks are not the next fix without reducing DA/RBC/QC/application tail
  latency and queue-drain cost. Keep the simple-transfer batch path guarded by
  exact trigger-filter matching so per-transaction transcript, event, trigger,
  and rejection semantics remain intact.
- Treat 20k committed TPS as a separate throughput goal from 20k ingress
  liveness. At 2-3s blocks, the current safe cap can only commit hundreds of
  transactions per second; reaching 20k committed TPS requires safe payloads in
  the tens of thousands of transactions per block, equivalent deterministic
  parallel execution, or both. Use the matrix runner for every optimization
  step and require the consensus liveness gate to stay green before accepting a
  higher-throughput result.
- Keep hardening the actor-owned vNext round state now that the standalone
  runtime reactor boundary is gone. vNext control frames, body-backed proposal
  acceptance, DA/RBC availability handoffs, timeout ticks, validation worker
  dispatch/start/result, proposal-backed validation gates, validation
  accept/reject/defer handling, re-chain/view-change aggregation, sidecar
  replay, and commit-persistence completion now run directly through `Actor`.
  Block-sync BlockCreated recovery now also uses named payload-only,
  requested-payload, signed-quorum, and commit-evidence recovery modes instead
  of broad stale/authoritative/revival bypass booleans. The remaining work is
  to delete any legacy cooperative commit sweep paths that become redundant
  once the actor-owned vNext state has equivalent model and integration
  coverage.
- Finish auditing chain-order hash and `rechain_seq` binding in deferred
  vote/QC caches, signer-tally/cache keys, and evidence replay paths used by
  the replacement shell. Vote/QC preimages, precommit signer history,
  block-sync-derived QCs, validator-checkpoint sidecars, raw/deferred vote
  caches, and vote/QC verifier cache keys now carry the selected binding.
- Reconstruct vNext chain order from committed/replayed re-chain and
  view-change certificates during catch-up. The live actor now keeps a bounded
  in-memory certificate journal, persists matching certificates into committed
  Kura roster sidecars, reloads those durable sidecars into outgoing
  `BlockSyncUpdate` payloads, and replays inbound sidecars before vote/QC
  processing. Vote/QC chain-order binding checks also hydrate matching durable
  sidecars from Kura before rejecting a `chain_order_hash`/`rechain_seq`
  mismatch. The remaining open work is to broaden catch-up model and
  integration coverage around restarted-peer durable sidecar replay.
- Add model and integration coverage for slow validation, queue saturation,
  malicious accusers, head failure during re-chain, NPoS stake-quorum
  quarantine edges, and DA/RBC loss during re-chain.

## Validation corridor

- Carry the Sumeragi NPoS/permissioned QC and VRF hardening through the next
  full workspace corridor.
  - A 2026-05-05 workspace rerun exposed three remaining
    `consensus_and_da` cases after the UAID replay/checkpoint fixes:
    stale evidence persistence, NPoS baseline timing, and late VRF reveal
    penalty recovery. The stale-evidence and NPoS performance focused reruns
    are green after the Torii horizon filter and baseline budget update. The
    late-reveal path now has code-level fixes for VRF vote-queue routing and
    deferring committed-block catch-up until after VRF metadata handling,
    epoch-record hydration before reveal validation, stale pending-seal
    retention, and external Torii VRF metadata gossip. The focused core units
    for those paths are green. The remaining integration blocker is now a
    separate four-peer NPoS/DA liveness stall: the late reveal is accepted in
    Sumeragi status, but the network repeatedly stalls at height 4 with RBC
    READY/DELIVER data waiting on missing INIT/chunk state before the pending
    VRF seal can be committed. Fix that h4 DA/RBC stall, then rerun
    `sumeragi_randomness::npos_late_vrf_reveal_clears_penalty_and_preserves_seed`
    as the final persistence gate.
  - Focused commit, block-sync, VRF, QC-validation, roster-selection, Torii VRF
    OpenAPI/parser, and data-model consensus roundtrip tests are green as of
    2026-05-02 with `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-verify`.
  - Additional Sumeragi/DA adversarial coverage is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor` for the debug
    witness-root unit, witness-corruption recovery, chunk-drop recovery,
    Kura-eviction DA rehydration, and block-body DA rehydration focused
    reruns. The remaining broad-run Sumeragi DA payload-loss case is also green
    as of 2026-05-03 with the same target dir.
  - NewView QC `highest_qc` binding, exact local-vote `highest_qc` and
    parent/post-root matching, non-NewView `highest_qc` rejection, and
    same-highest aggregate formation are green as of 2026-05-03 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest` for the NPoS
    aggregate-only substitution regression, the `new_view_highest` focused
    slice, and the stale/future NewView QC formation regressions. The same
    target now also covers commit/checkpoint missing-PoP rejection, block-sync
    QC validation with commit-phase enforcement, commit-certificate roster
    validation, checkpoint roster validation, validation telemetry reason
    labels, and the permissioned/NPoS aggregate-fallback quorum checks.
    Embedded commit-QC roster anchoring is green as of 2026-05-04 in the same
    target for both the malicious shrink-roster rejection and the valid
    stale-cache bootstrap path; the embedded-roster missing-PoP rejection is
    green in that same filter. NPoS block-sync roster selection now also has
    focused coverage for carrying a locally resolved stake snapshot when the
    incoming QC/checkpoint hint omits one.
  - The ZK-confidential localnet submit helper has been hardened for startup
    transport jitter and wrapped policy rejections. The classifier/retry-budget
    tests plus disabled shield/unshield localnet regressions are green as of
    2026-05-03 with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`. The full
    serial `consensus_and_da` target is also green in the same target dir:
    `250` passed, `0` failed, `6` ignored. Focused strict clippy over
    `iroha_core`, `iroha_torii`, `iroha_test_network`, and the
    `consensus_and_da` test target is also green in that target dir.
  - Focused `cargo clippy -p iroha_core -p iroha_data_model -p iroha_torii -p
    irohad --all-targets -- -D warnings` is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-clippy`.
  - Full workspace all-target clippy is green as of 2026-05-03 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
  - The broad workspace test rerun reached `events_and_triggers` after passing
    `consensus_and_da` and `core_api`; the exposed by-call trigger fixture and
    subscription time-trigger billing failures are repaired as of 2026-05-03.
    Focused `events_and_triggers` reruns for the two by-call trigger cases and
    `subscriptions::subscription_scenarios` are green with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
    The full `events_and_triggers` target, full `queries_and_proofs` target,
    `network_functional::extra_functional::unstable_network`, full
    `nexus_and_streaming` target, and reduced-sample ignored
    `torii_load_profile` are also green as of 2026-05-04 in the same target
    dir. The stale IVM/Kotodama, Space Directory, lane commitment, Norito
    instruction, and streaming RANS fixtures uncovered by those targets have
    been regenerated.
    The full `core_api` target is green again as of 2026-05-04 after repairing
    private-entrypoint hash handling and widening the slow asset/sealed-reveal
    liveness paths (`171` passed, `4` ignored).
    A broad `cargo test --workspace` reached `integration_tests --lib` after
    compiling the workspace and passing the preceding crate/test targets; the
    first integration-library pass failed on a stale spawned daemon artifact,
    then the exact startup/drop regressions and the full integration library
    passed after rebuild (`41` passed). The core signature slice, crypto
    Ed25519 tests, and strict clippy for core/crypto/integration are also green
    after the deterministic single-Ed25519 verifier cleanup and heartbeat
    execution-context fixture repair. The replay/checkpoint follow-up is green
    as of 2026-05-05 for the focused replay units, Halo2 restart-marker
    verifier, strict core/crypto/consensus integration clippy, and the
    previously failing `consensus_and_da` restart/localnet cases:
    `sumeragi_restart_retains_lock_convergence`,
    `npos_pacemaker_resumes_after_downtime`,
    `confidential_combined_peer_downtime_and_timeout_pressure_localnet`, and
    `confidential_dual_restart_stress_mid_flow_localnet`.
    The 2026-05-07 follow-up also has focused green reruns for the staged
    consensus failures exposed in the latest broad workspace attempt:
    selective-drop recovery, conflicting-ready invalidation, Kura eviction DA
    rehydration, NPoS baseline metrics, pacemaker latency, pacemaker restart
    liveness, stale-evidence rejection, and the VRF randomness module. The
    focused `integration_tests --test consensus_and_da` compile check is green
    in `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check`.
  - Remaining validation: rerun `cargo test --workspace` from a clean start to
    completion in an uncontended multi-hour window.
  - Broad workspace all-target compile validation is green as of 2026-05-07
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check` after
    repairing the default Linux monitor synth gate and stale
    `LaneBlockCommitment` fixture initializers in Python/`xtask`.
- Carry the RAM-LFE API/proof hardening through the remaining signing and clean
  full-workspace Cargo corridor.
  - Focused OpenAPI detached-envelope tests, crypto RAM-LFE tests, the new
    state-deserialization policy regression, Torii RAM-LFE handler tests,
    JavaScript RAM-LFE tests, Swift execute-response parsing, the focused
    `iroha_core` RAM-LFE gate, the workspace all-target compile corridor,
    focused strict clippy over the repaired tool/SDK/Mochi/CoreHost/proof
    targets, JavaScript/Kotlin/JVM/Java Android identifier BFV parity,
    JavaScript Connect Norito schema-hash parity, Android Norito schema-manifest
    verification, C# SDK tests on macOS with a temporary .NET 8 SDK, full Swift
    package tests, full workspace all-target clippy, `scripts/check_no_scale.sh`,
    formatting, and diff whitespace checks are green as of 2026-05-02.
  - Remaining validation: run `cargo test --workspace` in an uncontended
    validation window.
  - Windows C# follow-up: on a Windows box with .NET 8, run
    `dotnet restore csharp/Hyperledger.Iroha.Sdk.sln`, then
    `dotnet test csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj`.
    Confirm the canonical Norito schema-hash test, transaction-builder goldens,
    faucet PoW vectors, and URL escaping expectations pass unchanged; record the
    Windows result in `status.md`.
    Repo-local Linux validation and the read-only public Taira live smoke are
    green as of 2026-05-19; this item is now specifically the external
    Windows-host rerun.
    Also cover the new multisig propose helper work on Windows: the focused
    tests should include
    `ToriiClientTests.ProposeMultisigAsyncPostsNativeNoritoInstructionFrames`
    plus malformed-response cases for invalid or empty `signing_message_b64`,
    false `ok`, negative `creation_time_ms`, and malformed hash metadata,
    and `NoritoCodecTests.EncodeWithSchemaHashUsesProvidedSchemaHash`, and the
    review should confirm `TransactionInstruction.EncodeInstructionBoxBase64`
    emits `InstructionBox` frames suitable for `/v1/multisig/propose`.
  - Focused Kotlin/JVM and Java Android RAM-LFE parser/transport tests are
    green as of 2026-05-02 with Homebrew OpenJDK 21 pinned via `JAVA_HOME`; the
    same harnesses also cover the canonical BFV identifier schema-hash vector.
  - The current static OpenAPI manifest now verifies in explicit unsigned
    first-release mode; before publishing a signed OpenAPI release, rerun the
    same manifest flow with the operator signing key or detached Ed25519
    signature envelope.
- Carry the FastPQ V1 release hardening through the remaining broad validation
  corridor.
  - The 2026-05-17 implementation removes prover-scale CPU replay from
    verification, validates proof-carried roots/transcript challenges/Merkle
    openings/AIR rows/lookup-product binding/FRI query chains from proof
    content, defaults production runtime config to explicit `cpu`, fails
    explicit `gpu` startup closed when preflight is unavailable, bounds Kura
    FASTPQ proof sidecar persistence, adds sidecar telemetry, exposes
    `/v1/pipeline/recovery/{height}/fastpq-proofs`, and adds AXT packaging
    helpers for already-bound batches.
  - Focused `fastpq_prover`, `iroha_config`, `iroha_core fastpq`, Torii recovery
    endpoint, confidential localnet restart/recovery, and explicit
    `fastpq-gpu` release checks are green as of 2026-05-17 with
    `CARGO_TARGET_DIR=target/codex-fastpq-release` and
    `CARGO_TARGET_DIR=target/codex-fastpq-gpu`. The full workspace all-target
    clippy corridor is also green; the remaining open work is only the next
    multi-hour `cargo test --workspace` corridor.
  - AXT proof envelopes now require FastPQ V1 verifier labels at both the
    production FastPQ binding layer and the standalone IVM host envelope-shape
    layer. DefaultHost, CoreHost, and WSVHost reject raw proof bytes and
    synthetic/non-V1 proof labels during diagnostic preflight. Because
    standalone IVM does not link a real FastPQ verifier, proof-consuming AXT
    calls fail closed after preflight; the production AXT verifier rejects oversized
    encoded proof payloads before Norito decode. The descriptor-derived
    synthetic AXT batch builder and CLI fallback have been removed; proof
    generation and measurement require an execution-captured `batch_base64`
    request field. Core state no longer synthesizes FastPQ batch hashes for
    ad-hoc transcript/RWA contexts; those paths require transaction call-hash
    context or a trigger-specific call hash. The shared preflight checks also
    require concrete binding fields,
    supported FastPQ claim types, 32-byte hex digests, and nonempty
    duplicate-free target dataspace sets. DefaultHost also binds handles to the
    manifest root carried by inline, recorded, or late proof envelopes before
    failing closed without a verifier. The focused `fastpq_prover` AXT binding slice,
    `iroha_data_model` `proof_matches_manifest` slice, `ivm_abi`
    `preflight_fastpq_v1_proof_envelope` test, `ivm` `axt_host_flow` target, and
    `ivm` `host_unknown_syscall`/`core_host_policy` targets are green as of
    2026-05-02.
  - CoreHost raw-root rejection and real FastPQ proof-envelope validation is
    covered by the focused `ivm_corehost_axt` proof-binding test with
    `iroha-core-tests,app_api`; the correctly-featured target is green as of
    2026-05-03 with `28` tests. The `app_api`-only command lists zero tests and
    should not be treated as coverage for this target.
  - Block-level app-API AXT validation and host proof-cache success fixtures now
    use reusable FastPQ-backed proof envelopes. The full
    `axt_validation_tests` module and focused `axt_verify_ds_proof` host sweep
    are green as of 2026-05-02.
  - Shared `ProofBlob` matching, standalone `ivm` CoreHost/WSV tests, state
    replay-ledger fixtures, ISI lane-relay registration, and data-model AXT
    fixtures now reject raw manifest roots and binding-less success envelopes;
    only malformed-negative tests keep those payloads.
  - Lane relay proof metadata has no legacy deterministic digest helper and
    carries a required `verified_at_height` field. Verified lane-relay
    registration binds the envelope digest to the submitted proof blob payload
    hash; the data-model proof-material tests and core `lane_relay` slice are
    green as of 2026-05-02.
  - Replace the current prover-scale canonical replay verifier with a succinct
    quotient-only verifier once the V1 quotient commitment/opening API lands;
    this is a performance follow-up, not permission to accept synthetic AXT or
    placeholder proofs.
- Carry the SoraNet VPN escrow hardening through the remaining ledger and
  deployment corridor.
  - The Torii/relay/helper control plane now requires XOR quote payments,
    non-operator escrow custody, client usage vouchers, one-use helper tickets,
    relay TLS pinning, helper-ticket-bound metering keys, and tariff-derived
    relay settlement.
  - Native lease escrow ISIs, WSV lease records, verified tariff settlement,
    relay/helper streaming voucher debt-window enforcement, Torii native
    `OpenVpnLeaseEscrow` quote skeletons, and Torii native `SettleVpnLease`
    receipt skeleton responses through the generic `tx_instructions`
    tooling convention are implemented. Torii active-session lookup and receipt
    settlement now reload authoritative lease state from WSV instead of relying
    on process-local VPN session caches.
  - Relay/backend deployment now uses `vpn.backend_endpoint`; Unix sockets are
    the default privileged path, while TCP requires a shared bootstrap secret
    and Norito MAC envelopes with timestamp/nonce replay checks.
  - Hidden helper workers now receive magic-prefixed Norito connect-payload
    frames over stdin and batch magic-prefixed Norito traffic-state persistence.
  - Relay operators can set `vpn.receipt_spool_dir` to persist the exact
    `/v1/vpn/receipts` request body for voucher-backed sessions, so settlement
    no longer depends on reconstructing receipt bytes from logs.
  - `soranet-vpn-settlement` consumes those artifacts and signs deterministic
    Torii receipt headers/body, or renders curl, using runtime-only operator seed
    material.
  - The JavaScript, C#, Swift, Python, Kotlin/JVM, and Java Android Torii
    clients now expose the quote-first open flow and operator receipt
    submission helpers with native instruction skeletons.
  - Next, finish the focused Cargo validation once the current shared target
    locks clear, then run a public relay/helper/Torii canary that opens a native
    XOR VPN lease from the wallet flow, submits a spooled operator receipt, and
    signs the returned `SettleVpnLease` transaction.
- Carry the IVM/Kotodama vector and syscall hardening through the next clean
  validation corridor.
  - `cargo test -p ivm_abi`,
    `cargo test -p ivm --test vector_execution_regression`, and
    `cargo test -p kotodama_lang vector_length` are green as of 2026-05-02.
  - The updated IVM gas/metadata/pointer window is also green as of
    2026-05-02:
    `cargo test -p ivm --test gas_conformance --test gas_golden --test metadata --test metadata_roundtrip --test pointer_tlv_neg`.
  - The focused analyzer regression
    `cargo test -p ivm analysis_treats_setvl_operand_as_immediate --lib` is
    green as of 2026-05-02.
  - The SCALLX ABI expansion is green as of 2026-05-02 for
    `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing`,
    `cargo test -p ivm --lib scallx_dispatches_extended_syscall_id`,
    `cargo test -p ivm --test abi_hash_versions --test gas_schedule_hash --test syscalls_doc_sync --test ivm_abi_doc_sync`, and
    `cargo test -p ivm_abi --lib syscallx_roundtrips_24_bit_number`, all with
    `CARGO_TARGET_DIR=target/codex-ivm-scallx`; the core admission regression
    `cargo test -p iroha_core validate_ivm_unknown_scallx_rejected_at_admission --lib`
    is green with `CARGO_TARGET_DIR=target/codex-core-scallx`.
    Follow-up host-bound coverage
    `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing`,
    `cargo test -p ivm --lib run_with_host_accepts_non_sync_host`, and
    `cargo test -p ivm --lib block_height_syscall_uses_configured_deterministic_value`
    is green with `CARGO_TARGET_DIR=target/codex-ivm-scallx`. Core host
    coverage for `dedicated_query_syscalls_return_norito_payloads`,
    `block_height_sysvar_uses_attached_transaction_context`, and scoped
    durable-state `STATE_KEYS`/`STATE_HAS`/`STATE_LEN`/`STATE_COUNT`
    tombstone resolution is
    green with `CARGO_TARGET_DIR=target/codex-core-scallx`.
    Broader IVM validation is also green with
    `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib` plus
    targeted integration batches for gas/opcode/vector, metadata/pointer ABI,
    predecoder/doc sync, syscalls, WSV-host flows, VRF, and ZK verifier gates.
  - Dedicated `QUERY_GET_ACCOUNT`, `QUERY_GET_ASSET`,
    `QUERY_GET_ASSET_DEFINITION`, `QUERY_GET_DOMAIN`,
    `QUERY_GET_CONTRACT_MANIFEST`, `QUERY_GET_NFT`, `QUERY_GET_PARAMETER`,
    and `QUERY_GET_CONTRACT_INSTANCE` are implemented. The helpers either use
    the validated query engine or deterministic attached-state snapshots, and
    all charge the singular query gas model in code and generated docs.
    `SYSVAR_BLOCK_HEIGHT` is threaded through default hosts and attached core
    query-state contexts. `STATE_KEYS` now provides deterministic durable-state
    prefix enumeration with pagination and contract-scope prefix stripping.
    `STATE_HAS`/`STATE_LEN` provide cheap presence and payload-length probes,
    and `STATE_COUNT` counts matching durable-state keys without returning the
    key list over the same scoped durable-state resolution. Classic `STATE_GET`,
    `STATE_SET`, and `STATE_DEL` now charge documented deterministic state gas
    instead of returning zero. `GET_ACCOUNT_BALANCE` and
    `RESOLVE_ACCOUNT_ALIAS` also return deterministic nonzero query-style gas.
    `TLV_EQ` and `TLV_LEN` now charge deterministic byte-counted codec-helper
    gas costs instead of inspecting potentially large payloads for free.
    Numeric helpers now charge the fixed `G_numeric` cost across default, WSV,
    standalone codec, and real-host forwarding paths. `POINTER_TO_NORITO` and
    `POINTER_FROM_NORITO` now charge `G_pointer + bytes` across the default,
    WSV, and standalone codec hosts, with the byte component tied to the
    canonical TLV envelope copied or validated. Schema helpers and the
    remaining classic codec helpers now charge deterministic byte-counted gas:
    `SCHEMA_*`, `JSON_*`, `DECODE_INT`/`ENCODE_INT`, `NAME_DECODE`, and the
    path builders no longer return zero for payload work. `SM2_VERIFY` now
    charges `G_verify + bytes`; `SM4_GCM_*` and `SM4_CCM_*` now charge
    `G_sm4 + bytes` through the shared default-host implementation, preserving
    deterministic vector output while charging AAD and plaintext/ciphertext
    bytes. Deterministic sysvar reads now charge
    `G_sysvar` or `G_sysvar + bytes`, and authority reads charge
    `G_get_auth + bytes`, across default, WSV, standalone codec, and real-host
	    paths. VRF verification now charges `G_verify + bytes` on decoded
	    status-returning paths, and standalone/WSV ZK verification status exits now
	    charge payload-size verification gas instead of returning zero. ZK
	    roots/tally reads and VRF epoch-seed reads now charge request + response
	    byte gas across standalone, WSV, and real CoreHost paths.
	    `VERIFY_DS_PROOF` now charges `G_verify + bytes` in the real
	    smart-contract host and `G_verify` for proof-clear paths across real,
	    default, standalone CoreHost, and WSV mock hosts while standalone
	    proof-consuming AXT calls remain fail-closed without the real FastPQ
	    verifier. Runtime helper syscalls now also avoid documented zero-gas
	    gaps: `INPUT_PUBLISH_TLV`
    charges envelope bytes across default, WSV, and standalone CoreHost paths;
    `VERIFY_SIGNATURE` charges message/signature/key bytes; and private input,
    nullifier, output commit, heap growth, allocation shim, debug/exit/abort,
    validation-only ISI mutation stubs, FastPQ batch-entry/apply validation,
    and Merkle proof helpers return fixed, page, per-entry, or depth costs
    instead of zero. The WSV mock host direct mutation ABI surface, FastPQ
    transfer batch apply path, and development `SMARTCONTRACT_EXECUTE_QUERY` /
    `SMARTCONTRACT_EXECUTE_INSTRUCTION` JSON shims now also return deterministic
    query or mutation gas instead of treating mock-host state changes as free.
    The real smart-contract host now charges the declared `G_sc_depth` floor for
    `SET_SMARTCONTRACT_EXECUTION_DEPTH`, including the zero-depth no-op path,
    and the declared `G_create_nfts_all` floor for empty
    `CREATE_NFTS_FOR_ALL_USERS` snapshots.
    The classic hash helper surface now includes gas-charged SHA-256,
    SHA3-256, raw Blake2b-256, Keccak-256, and Iroha `Hash::new` syscalls
    routed through the real smart-contract host with byte-identical CPU or
    byte-equivalent accelerated output requirements.
  - `VERIFY_PROOF` now has a CoreHost implementation for
    `NoritoBytes(OpenVerifyEnvelope)` payloads backed by on-chain verifying-key
    registry prechecks and deterministic status-code returns; standalone IVM
    hosts continue to reject it without registry context. Acceleration status
    reporting now only marks CUDA parity as OK when the backend is usable after
    policy, hardware detection, and self-tests.
  - `PROVE_EXECUTION` now returns `NoritoBytes(ExecutionProof)` instead of a
    reserved stub. The proof summary commits to deterministic trace/log/root
    material with SHA-256 and is stable across repeated identical runs, while
    leaving room for later cryptographic prover backends to bind to the same
    public material. Focused unit, syscall, doc-sync, gas-doc, and `ivm_abi`
    regression checks are green with `CARGO_TARGET_DIR=target/codex-ivm-scallx`.
  - The broader `cargo test -p ivm` corridor is green as of 2026-05-02 after
    repairing the data-model compile blocker, refreshing the AXT fixture
    headers, and moving Kotodama test helpers to host-private SCALLX numbers.
    The optimized `cargo test -p ivm --test shifts_prop` focused rerun is also
    green.
  - The `ivm_contract_deploy` staged copy/register fixture tests are green as
    of 2026-05-07 after the literal-table padding repair:
    `cargo test -p iroha_cli --bin ivm_contract_deploy staged_ -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check`.
  - Follow-up widened checks are green as of 2026-05-02:
    `cargo test -p ivm_abi`, `cargo test -p kotodama_lang`,
    `cargo clippy -p ivm_abi -p kotodama_lang --all-targets -- -D warnings`,
    and `cargo clippy -p ivm --all-targets -- -D warnings`.
  - Fold the 2026-05-03 Kotodama access-hint, contract artifact registry, and
    literal-padding hardening through the next clean full workspace test and
    clippy corridor after the focused validation recorded in `status.md`.
  - Fold the 2026-05-03 IVM ABI v1 gas/error hardening through the next full
    workspace test and strict clippy corridor after the focused syscall-doc,
    host-policy, AXT, and Soracloud validation recorded in `status.md`.
- Carry the UAID onboarding hardening through the next workspace validation
  corridor.
  - Focused formatting, Python syntax checks, Torii UAID parser tests, Torii
    MCP shortcut/raw-body tests, Torii HTTP onboarding negative-contract tests,
    the full Torii onboarding integration target, Torii onboarding
    error-metadata tests, Swift register-account tests, focused IVM host
    thread-safety tests, the OpenAPI sync/version/signature script tests, the
    focused core UAID portfolio grouping test, the DA manifest fixture sweep,
    `cargo test -p iroha_torii --lib --features app_api`, and
    `cargo check --workspace --all-targets` are green as of 2026-05-02. The
    full workspace all-target clippy corridor is also green as of 2026-05-02
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
  - The Rust implementation is in place for explicit UAID-only onboarding,
    digest-only identity commitments, MCP/OpenAPI request contracts, Swift
    request canonicalization, asset-scope-aware UAID portfolio grouping, and
    stale OpenAPI manifest-signature suppression in generated version indexes;
    `versions.json` has been refreshed in explicit unsigned mode pending the
    operator signature.
  - Keep the broader `cargo test --workspace` corridor open. The repaired
    `events_and_triggers`, `queries_and_proofs`, `nexus_and_streaming`,
    unstable-network, and `core_api` targets are green individually as of
    2026-05-04. The Sora governance runtime-upgrade path now hashes prepared
    transaction entrypoints from the actual canonical signed payload bytes and
    confirms Torii status with explicit auto scope, but the full workspace
    command still needs an uncontended end-to-end pass. The static OpenAPI JSON,
    version index, and unsigned latest/current manifests are refreshed and
    verify under the explicit first-release unsigned corridor.
- Carry the Torii exposure-hardening slice through the next clean Cargo
  validation corridor.
  - `cargo fmt --all` and `cargo check -p iroha_config -p iroha_torii` are
    green as of 2026-05-02 for the CORS/pre-auth, MCP tool-effect,
    protected-namespace, route-catchall, mixed-content extractor, and router
    composition changes.
  - `cargo test -p iroha_config torii_cors_parse --lib` and
    `cargo test -p iroha_torii tool_effects --lib` are green as of
    2026-05-02. The follow-up MCP effect audit is also green for
    `cargo test -p iroha_torii get_tools_are_declared_read_effect --lib`,
    `cargo test -p iroha_torii manual_sumeragi_snapshot_tools_remain_read_only --lib`,
    and `cargo test -p iroha_torii tool_effects --lib` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-continue`. Fold the slice into the
    next workspace clippy/test corridor when validation budget allows.
- Carry the Torii first-release API cleanup through the remaining release
  corridor.
  - The route/API/error-envelope implementation, focused Rust sidecar/client
    tests, Swift/Python/Kotlin/JVM/Java Android/JavaScript client regressions,
    JS native/dist rebuild, formatting, and whitespace checks are green as of
    2026-05-17. Static OpenAPI JSON snapshots and latest/current unsigned
    manifests are refreshed and verified; the remaining broad release work is
    the next full workspace test/clippy corridor.
- Carry the Iroha Connect hardening through the remaining SDK and workspace
  validation corridor.
  - P2P session claims, hashed token storage, focused Rust checks, JavaScript
    checks, JS `dist`, Python syntax checks, and shared relay-auth vectors are
    green as of 2026-05-01.
  - Python pytest, Kotlin/JVM, Java Android, and Swift package tests remain
    blocked by missing local tools/artifacts.
  - When the validation shell has `pytest`, a Java runtime, and
    `dist/NoritoBridge.xcframework`, rerun the focused Python Connect tests,
    `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.connect.ConnectWalletRequestTest --console=plain`,
    the matching Java Android Connect wallet tests, and the focused Swift
    Connect/Torii tests.
  - Fold the Connect session/relay changes into the next broader
    `cargo test -p iroha_torii`, `cargo test --workspace`, and workspace clippy
    corridor when validation budget allows.
- Carry Offline real-proof support through the remaining release corridor.
  - The native bridge prover FFI focused corridor is green as of 2026-04-30. Fold it into a broader `cargo test -p iroha_core --lib`, SDK test, and workspace clippy corridor when validation budget allows.
  - Offline-to-offline SDK local-final semantics, trusted Ed25519 issuer
    certificate verification, and Android rollback fail-closed storage checks
    are green as of 2026-05-17 across Swift, Kotlin/JVM, Java Android, iOS
    simulator XCTest, and Android emulator instrumentation. Fold this into the
    next full workspace test/clippy corridor when validation budget allows.
  - The pure Swift Offline prover hot path is green as of 2026-05-01 with
    subsecond median native audit/redeem proofs on macOS arm64. Keep that
    benchmark in the next iOS-device corridor and broaden Swift package
    validation when budget allows.
  - Kotlin/JVM and Java Android now have the native Offline instance-value
    groundwork and pure Java Halo2/IPA prover path, including focused JVM and
    Android harness coverage plus env-gated benchmark hooks. Keep the native
    prover tests, Swift/JVM cross-verification payload, and larger benchmark
    iteration counts in the next device and full-SDK corridor.
  - The Torii Offline issuer hardening focused corridor is green as of
    2026-05-01. Fold it into the next broader `cargo test -p iroha_torii`,
    SDK, workspace test, and workspace clippy corridor when validation budget
    allows.
- Carry native asset escrow through the remaining Aitai application corridor.
  - Wire the Sora Aitai application UI/backend onto the native numeric escrow ISIs and proof-carrying anonymous escrow helper surfaces, then subscribe through the numeric and anonymous escrow query/event APIs.
  - Add app-facing lifecycle events for transparent and shielded offer state changes, and keep any remaining Kotodama wrapper work scoped to app calls that still need contract compatibility.
  - Add end-to-end UI/client smoke coverage once the Sora Aitai application replaces the old contract escrow account path for both transparent XOR and shielded anonymous-asset offers.
  - Rerun the full Kotlin, Java Android, and Swift SDK suites after the Aitai app wiring lands and a Java 21 runtime is available in the validation shell.
  - Keep NFT/RWA escrow and court fee/payout generalization as separate follow-ups; the v1 primitive intentionally resolves only between the escrow seller and accepted buyer.
- Carry the Soracloud production posture hardening through the operator-host rollout corridor.
  - Local focused, portable QEMU, and prior multi-peer load gates are green as of 2026-04-25; the readiness runner now reports missing operator inventory and missing observability evidence as production blockers. Before public rollout, run the mixed-host Inrou smoke with the real operator inventory, attach the real metrics/status/alert/dashboard evidence, and archive a blocker-free readiness report.
  - The full `irohad` Soracloud binary filter is green as of 2026-05-05 under
    `--features embedded-soracloud-runtime`. The full readiness profile still
    requires operator mixed-host inventory and observability evidence before it
    can produce a blocker-free rollout report.
  - The affected live deployment is intentionally running the 2026-05-08
    no-embedded-runtime `irohad` binary after the Inrou advert incident. Before
    any future live Soracloud runtime rollout, add an explicit operator config
    gate for Inrou enablement and prove that zero-backend hosts do not emit host
    adverts.
- Carry the new Taira devex CLI through the opt-in live rollout corridor.
  - The local CLI/Torii/mock-script validation for `iroha taira doctor` and `iroha taira write-canary` is green as of 2026-04-25, but no live Taira write was run from this tree.
  - Before publishing a live receipt, run `iroha taira doctor --public-root https://taira.sora.org` and an operator-approved `iroha taira write-canary --public-root https://taira.sora.org`, preserving only the redacted receipt and any stable failure codes.
  - Fold the Taira CLI/Torii changes into the next broader `cargo test -p iroha_cli`, `cargo test -p iroha_torii`, workspace test, and clippy corridor when validation budget allows.
- Carry the verified lane relay JSON-state/key change through the next UC6 integration corridor.
  - The focused crate checks are green as of 2026-04-24, but no live UC6 settlement-smoke run or topology reset has been performed from this tree.
  - Before any live deployment, confirm the deploy/Core API smoke path still uses `relay_state_key`, JSON relay state, and the simulation gate against the exact finalization payload.
  - If a topology plan selects reset mode while validating this change, stop before approval and reassess the rollout scope.
- Carry the Torii routed-read and telemetry fixes through the next workspace validation corridor.
  - The crate-local sweep is green as of 2026-04-24 with `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture`.
  - When validation budget allows, carry the alias-routing and Torii telemetry slices through the next `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor and record the result in `status.md`.
- Broaden validation for the new canonical account-alias lease flow beyond the focused onboarding and executor checks.
  - The onboarding auto-renew path now grants the subscriber `CanModifyNftMetadata` for the subscription NFT before trigger registration; rerun a wider `cargo test -p iroha_torii` window with the new `/v1/accounts/{account_id}/aliases`, `/renew`, and `/auto-renew` handlers enabled.
  - Add or rerun focused coverage for user-signed enable/disable mutation flows and the SNS subscription auto-renew billing path in `crates/iroha_core/src/smartcontracts/ivm/host.rs`, not just the onboarding enqueue path.
  - Once the alias lease slice is stable under those focused reruns, fold it into the next broader `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor.
- Keep the Sumeragi main-loop broad corridor attached to future consensus
  changes.
  - The 2026-05-08 idle timing-cache change is covered by focused cached
    commit-quorum timeout, NPoS commit-floor, rebroadcast-cooldown,
    commit-pipeline cooldown, effective-timing snapshot, mode-flip tick
    deadline, commit-evidence replay cooldown, and proposal-backpressure timing
    tests. Rerun the full `cargo test -p iroha_core --lib` corridor before the
    next consensus sweep.
  - The 2026-05-08 known-block commit-QC recovery dampening is covered by
    focused cert-only fetch, duplicate-QC cleanup, committed-tip reacquisition,
    stale same-height view pruning, bounded missing-QC view rotation,
    stall-reset fallback handoff, local-payload recovery, retry-loop, and
    same-height `BlockBodyResponse` repair tests. Rerun the full
    `cargo test -p iroha_core --lib` corridor before the next consensus sweep.
  - The realistic 30 TPS, 20-minute transfer soak passes as of the
    2026-05-08 block-body response ingress fix. Remaining open work is
    throughput margin, not liveness: the passing release-daemon run submitted
    at 30.00 TPS but committed 21.61 TPS during load, peaked at 9,973 queued
    transactions, and needed 722 seconds of drain time. Use
    `integration_tests/artifacts/realistic-30tps-transfer-20min-640-release-daemon-block-body-response-block-lane/throughput-1778229477740/`
    for the next worker/proposal throughput tuning pass.
  - The matching realistic 30 TPS, 20-minute RAM-LFE email-claim soak also
    passes as of 2026-05-08. The release-daemon run submitted 36,000
    `ClaimIdentifier` email transactions, reached the 36,008 approved target
    with zero rejects, and finished with all peers at 723 non-empty blocks.
    Margin remains the open item: load committed at 21.27 TPS, final committed
    TPS was 19.17 including drain, peak queue was 10,377, and drain took 677
    seconds. Use
    `integration_tests/artifacts/realistic-30tps-ram-lfe-email-20min-release-daemon/throughput-1778232961671/`
    alongside the transfer artifact for the next worker/proposal throughput
    tuning pass.
  - The 2026-05-08 DA/RBC large RAM-LFE proposal fallback is covered by focused
    DA payload-budget tests, a RAM-LFE oversized-frame fallback regression, and
    the adjacent unservable-payload deferral check. Rerun the full
    `cargo test -p iroha_core --lib` corridor before the next consensus sweep.
  - The 2026-05-06 canonical proposal/block entrypoint-ordering fix is covered
    by focused ordering, mixed-entrypoint builder, rejection mapping,
    noncanonical static/unchecked-validation, and PrivateKaigi entrypoint
    execution regressions. Rerun the full `cargo test -p iroha_core --lib`
    corridor before the next consensus sweep.
  - The 2026-05-03 `cargo test -p iroha_core --lib` rerun is green
    (`5129` passed, `22` ignored) after fixing execution-witness recorder
    isolation and hardening the RBC sidecar cooldown fixture.
  - The later 2026-05-03 restarted-peer commit-QC recovery fix is covered by
    focused block-body response regressions and the confidential downtime plus
    timeout localnet scenario, now passing without the restarted-peer catch-up
    waiver warning. Rerun the full `cargo test -p iroha_core --lib` corridor
    after the next main-loop edit or before opening the next full workspace
    sweep.
  - For the next consensus change, rerun the same broad window so the collector
    fallback, exact-frontier repair, cached-target, vote replay, roster
    recovery, future-new-view, and model-backed reschedule fixtures continue to
    execute together rather than only as isolated filters.
- Broaden Sumeragi verification when new fatal hang classes are identified
  outside the current two-slot frontier abstraction.
  - The 2026-05-03 frontier formal process hardening is green and covers active
    pending progress touch, local-vote and commit-QC progress, stale recovery
    subject-view scope, vote-queue drain, payload recovery, quorum retransmit,
    retransmit follow-through, and future-slot promotion.
  - For any additional fatal hang shape, first add a focused Rust regression,
    then add the corresponding finite formal dimension or mutation so the
    expected-failure suite proves the model would have caught it.
  - If another restarted-peer catch-up issue appears in message admission or
    deduplication, add a small finite admission-order bridge or mutation before
    broadening the frontier model itself; the current model intentionally
    abstracts network-message dedup away.
  - Keep this scoped to the observed hang surface; do not generalize the model
    into an arbitrary pipeline unless a new bug requires more than the active
    plus one-future-slot abstraction.
- Reopen the wider validation corridor after the recent focused `iroha_core`, `iroha_torii`, and `iroha_data_model` test additions.
  - `cargo test -p iroha_core --lib` is green as of 2026-05-03; rerun it only
    after the next core/consensus change or before opening the full workspace
    corridor.
  - `cargo test -p iroha_torii` is green as of 2026-05-03 after fixing the
    macOS attachment-sanitizer subprocess wrapper path; rerun it after the next
    Torii/API change or before opening the full workspace corridor.
  - Rerun `cargo test -p integration_tests -- --nocapture` once the current
    tree is stable enough for network suites.
  - When validation budget allows, rerun `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`, then capture failures or green status in `status.md`.
## Consensus and Izanami

- Maintain Izanami communication vulnerability publication evidence.
  - The exact-injector 75% packet-loss 2026-04-26 paper-shaped run at `dist/izanami-exact-packet-paper-20260426` is green for both permissioned and NPoS Sumeragi and is recorded in `status.md`; keep this as the current full-matrix resilience baseline.
  - Native in-process P2P packet-drop injection is wired into `packet-loss` and leader-targeted `leader-isolation`; the matrix runner now supports the paper's 133s-266s timed fault window plus configurable packet-loss sweeps (`75%` quick, `25%/50%/75%` paper). The explicit 25%/50%/75% paper packet-loss sweep at `dist/izanami-packet-sweep-paper-20260427-loss-only` is green for both permissioned and NPoS Sumeragi and is recorded in `status.md`.
  - The 2026-04-27 quick matrix at `dist/izanami-quick-both-20260427` is green for all ten permissioned/NPoS rows, and the post-ingress-hardening leader-isolation rerun at `dist/izanami-quick-leader-retry-20260427` keeps both modes resilient with zero acceptance markers.
  - The result-strengthened matrix and sweep tooling is implemented as of 2026-04-28, including bounded shutdown-drain accounting, latency/recovery evidence, NPoS repair-coverage telemetry, generated `paper-style-final-report.md`, and separate `stress-400` / `stress-800` profiles.
  - Seed-7 real stress evidence at `dist/izanami-stress-400-seed7-20260428` and `dist/izanami-stress-800-seed7-20260428` is refreshed and green as of 2026-04-29: both `stress-400` and `stress-800` report 14/14 resilient rows across permissioned and NPoS Sumeragi, with no real `confirmation_queue_dropped` pressure in the fresh artifacts. This is recorded in `status.md`.
  - Run the full paper/stress seed sweep with fresh binaries when validation budget allows: `scripts/run_izanami_communication_vulnerability_sweep.sh --profiles paper,stress-400,stress-800 --sumeragi-mode both --seed-list 7,11,13,17,19,23,29,31,37,41`. Paper rows must remain `resilient`; stress rows should stay reported separately as margin evidence across broader seeds.
  - Keep any future publication reruns split with `--sumeragi-mode both` so permissioned and NPoS Sumeragi classifications are not collapsed, and preserve per-loss packet-loss subrows when comparing against the paper's Algorand/Aptos/Avalanche/Redbelly/Solana baseline.
- Recalibrate the Izanami stable-profile acceptance envelope for sustained workload targets.
  - The fresh 4-peer permissioned `1 TPS` / `300s` / `100 blocks` gate at `dist/izanami-stable-gate-20260427-target100` is green and recorded in `status.md`.
  - The matching `200`-block diagnostic at `dist/izanami-stable-gate-20260427-rerun` crossed the prior stall region and reached strict/quorum height `107` with zero submission or confirmation failures, but missed the target because the stable workload drained before `200` blocks.
  - Before the longer `3600s` / `2000+` block acceptance pass, choose a sustained-workload gate or lower short-run target so the KPI measures liveness instead of exhaustion of submitted work.
- Root-cause the remaining NPoS soak/localnet collapse instead of keeping it as a log-only symptom.
  - Reproduce with preserved peer dirs and `iroha_futures::supervisor=debug`.
  - Identify the first exiting supervised child before investigating downstream connection refusals.
  - Cross-check peer logs with `/v1/sumeragi/status` counters so the fix targets the actual failing layer.

## Throughput and query performance

- Re-establish current throughput knees for the de-amplified harness and shared-host localnet.
  - Rerun the stepped single-host sweep.
  - Repeat permissioned and NPoS passes on the same hardware envelope and compare against the archived `25-50 TPS` / `75-100 TPS` baselines.
  - Record the new knee points and any regressions in `status.md`.
- Carry the 2026-05-02 Norito/Crypto scalar hot-path slice through the remaining
  release validation corridor.
  - The Ed25519 admission follow-up now caches deterministic 32-byte invalid
    public-key parse outcomes, routes compact/full conversion through the
    cached parse path, widens the hot thread-local parse/verify caches for the
    20k stable workload window, skips signature parsing and dalek batch setup
    for all-cached exact verify tuples, and preserves lowest-original-index
    failure reporting for mixed batches. Focused crypto/Torii checks and the
    `ed25519_hotpaths` Criterion bench are recorded in `status.md`.
  - Remaining local benchmark baselines: `cargo bench -p iroha_data_model
    --bench chain_wire`, `cargo bench -p iroha_data_model --bench
    decode_registry`, and `cargo bench -p iroha_core --bench crypto_hotpaths`.
  - The latest 120s release gate rerun exists at
    `dist/izanami-prebuilt-20k-rerun-release-ed25519-cache-120s-20260502-180614`
    and is recorded in `status.md`; the wrapper exited `0`, but it is not a
    clean all-accepted ingress gate. It offered all `2,400,000` planned
    submissions, accepted `2,364,756`, reported `35,244` failures, and reached
    strict approved transactions `20,582` at strict height `7`, with the queue
    still saturated. Active build/gate process lines were captured before and
    after the run, so it remains diagnostic evidence only.
  - The latest contended 30s sampled profile exists at
    `dist/izanami-profile-20k-ed25519-cache-sampled3-30s-20260502-182524`
    and is recorded in `status.md`. It submitted and accepted all `600,000`
    planned ingress attempts but only reached strict approved transactions
    `4,113` at strict height `3`, with the queue still saturated. The next
    bottleneck focus remains peer CPU: FASTPQ transcript finalization over
    Norito account/numeric/array serialization into Poseidon byte hashing;
    Ed25519/Curve25519 batch-verifier miss work and public-key parse/decode
    misses; Norito transaction/signature decode and compact-length work; and
    smaller allocation/copy/CRC64 costs. It is not a clean comparable baseline
    because workspace `cargo test`/rustc and another debug test network were
    active before and after the run.
  - The FASTPQ GPU follow-up is now recorded in `status.md`: Metal toolchain
    preflight is green, `bn254_poseidon_words` uses the Metal backend,
    transcript digest finalization overlaps Metal dispatch with CPU work,
    execution-witness digest propagation avoids a duplicate witness-side
    finalization, the final `fastpq-gpu` 120s release gate accepted all
    `2,400,000` offered submissions and reached `36,986` strict-approved
    transactions, and the delayed load-window sampled peer stacks have no
    scalar `poseidon3_permute` or CPU FASTPQ fallback. CUDA hardware closure
    evidence was captured later on 2026-05-19 and is recorded in `status.md`.
  - The 2026-05-05 hardware-backed FASTPQ Metal parity rerun on macOS is green
    after repairing Goldilocks FFT/LDE, BN254 LDE, and Poseidon Metal/CPU
    mismatches. CUDA hardware closure evidence was captured later on
    2026-05-19 and is recorded in `status.md`.
  - The next throughput slice should target the post-GPU peer CPU stack:
    Ed25519/Curve25519 public-key parse and verification, Norito
    transaction/transfer serialization and decode, transaction metadata
    hashing, allocation/copy traffic, and CRC64/SHA-256 helpers. The first
    bookkeeping slice already removed per-transaction `DashMap::len()` from
    `PipelineStatusCache::prune_if_needed`, and the Ed25519 thread-local slice
    now includes a direct-mapped full-key cache before the generic linear
    verifier cache. The current allocation slice streams typed Norito hashes
    directly into Blake2b, finalizes direct Blake2b hashes into fixed buffers
    without boxed digest allocation, absorbs Merkle parent/commitment chunks
    without staging concatenation buffers, and hashes external transaction
    entrypoints through a borrowed encoder instead of cloning the signed
    transaction into an enum wrapper. The release Izanami/iroha3d binaries now
    rebuild with the allocation slice, and the clean return gate at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-120s-20260504-012106`
    restored ingress (`2,400,000` accepted and succeeded, `0` failures) but
    still reached only `12,413` strict-approved transactions at height `5`.
    The matching sampled profile at
    `dist/izanami-profile-20k-fastpq-gpu-return-sampled-30s-20260504-012521`
    was intrusive, but its peer stacks confirm the next work remains
    Ed25519/Curve25519 parse and verification, Norito transaction/transfer
    encode/decode, metadata hashing, allocation/copy traffic, and SHA-256/CRC64
    helpers. A first queue-lock slice now releases `push_remove_lock` before
    post-enqueue backpressure/gossip/event/wake side effects. The follow-up
    bottleneck fix repairs the post-queue-lock execution-context mismatch,
    moves RBC READY/DELIVER traffic onto the consensus-chunk lane, gives chunk
    traffic a turn after each high-priority payload frame, caches prepared
    metadata JSON depth, and keeps prepared metadata depth checks on the
    static-validation hot path. The clean rebuilt
    `20k TPS` / `120s` `fastpq-gpu` gate at
    `dist/izanami-prebuilt-20k-fastpq-gpu-bottleneckfix-120s-20260504-183724`
    accepted and succeeded all `2,400,000` submissions with no safety failures
    and reached `37,000` strict-approved transactions at height `11`, but queue
    saturation remained (`854,344 / 2,400,000`). The matching sampled profile at
    `dist/izanami-profile-20k-fastpq-gpu-bottleneckfix-peer-sampled-30s-20260504-184154`
    shows no scalar FASTPQ/Poseidon fallback; the next bottlenecks are block
    validation and serialization costs: Ed25519/Curve25519 verification math,
    Norito compact-length and transaction/transfer encode/decode,
    allocator/reallocation and copy traffic, SHA-256/Blake2/CRC64 helpers,
    `resolve_streaming_metadata`, and pipeline access/overlay preparation.
    A final prepared-hash cleanup after that profile avoids temporary
    signed-transaction byte vectors while preparing hashes/lengths and reuses
    prepared payload/signed hashes in validation cache and signature-batch
    paths. The current-code `20k TPS` / `120s` rerun at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260504-194602`
    covered that cleanup: Izanami exited `0`, accepted and succeeded all
    `2,400,000` submissions, recorded no safety failures, and had submit
    latency `p50=6ms`, `p95=21ms`, `p99=99ms`, `max=269ms`. Strict progress
    was lower than the previous gate at `32,956` approved transactions at
    height `10`, with queue saturation still high (`883,791 / 2,400,000`) and
    commit-pipeline EMA `592ms`. Treat the 20k ingress path as restored; the
    committed-throughput target still needs the next validation/serialization
    hotspot pass. The fresh current-code profiles refine that target: the
    immediate `30s` sample at
    `dist/izanami-profile-20k-fastpq-gpu-current-peer-sampled-30s-20260504-195325`
    shows FASTPQ Metal pipeline creation still happens on the first proof hot
    path, while the delayed post-warm `60s` sample at
    `dist/izanami-profile-20k-fastpq-gpu-current-peer-postwarm-sampled-60s-20260504-195720`
    moves the steady-state bottleneck back to validation and serialization.
    The 2026-05-05 FASTPQ lane preflight follow-up moves backend construction
    off the startup/submission path, keeps digest acceleration disabled until
    the lane observes successful GPU preflights, and falls back to CPU prover
    modes after a failed Poseidon GPU preflight. The current May 6 return gate
    at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260506-124641`
    accepted and succeeded all `2,400,000` submissions with no safety failures
    and reached `49,428` strict-approved transactions at height `14`, above the
    previous `45,191` preflight gate. Treat first-proof FASTPQ GPU preflight
    and the latest single-transfer digest deferral path as addressed for now;
    the next open work is Ed25519/public-key parse and verify work, Norito
    transaction and transfer encode/decode/length accounting, allocation/copy
    churn, queue-admission/world-view preparation, and queue drain under
    saturated 20k ingress. That older profile avoided scalar FASTPQ/Poseidon
    fallback work until new evidence; the May 7 load-window sample below
    reintroduces scalar cost specifically in the BN254 runtime digest path,
    while general FASTPQ prover parity remains fixed.
    The 2026-05-07 Metal final return gate fixes general FASTPQ Poseidon
    preflight parity and removes normal commit-QC inline validation supersedes;
    keep the next Izanami pass on queue drain/block-validation cost and BN254
    runtime Metal batch stability, not on prover Poseidon preflight parity.
    The corrected load-window profile at
    `dist/izanami-profile-20k-fastpq-gpu-final-loadsample-90s-20260507-225637`
    sharpens that order: scalar Halo2 BN254 Poseidon is again the top sampled
    application leaf after runtime Metal batch failures, while consensus
    progress is limited by payload availability and exact-frontier recovery
    signals under a saturated queue. Fix BN254 runtime batch stability first,
    then reduce local READY/DELIVER deferrals and block-body reacquisition
    latency before revisiting the secondary Norito, Ed25519/Curve25519, SHA-2,
    Blake2, CRC64, and allocation hot paths.
  - Avoid repeating the rejected process-wide Ed25519 public-key parse cache
    approach without new evidence: the 2026-05-03 sharded shared-cache
    experiment regressed short-gate commit progress and was backed out. Keep
    near-term Ed25519 work thread-local, allocation-focused, or validation-path
    specific unless a clean before/after gate proves otherwise. The accepted
    thread-local slice pre-sizes only the public-key parse map, keeps parsed key
    entries boxed to satisfy `variant-size-differences`, and keeps the generic
    verify-ok map lazy so 32-byte transaction hashes do not allocate unused
    generic cache state.
  - Keep broader trait-wide parallel decode, deeper GPU decode materialization,
    deeper dalek backend experimentation, and deterministic hardware-specific
    Ed25519/Curve25519 acceleration as follow-up work until the current
    bottleneck slice has clean before/after evidence.
- Continue the 20k post-cache throughput tuning corridor.
  - The first post-cache 4-peer no-fault prebuilt `20k TPS` / `120s` release
    gate at `dist/izanami-prebuilt-20k-hotpath-120s-20260501-142015` improved
    strict approved transactions to `28,713` but still failed the committed
    20k target.
  - A same-shape repeat at
    `dist/izanami-prebuilt-20k-cachepass-120s-20260501-142429` accepted
    `52,167` ingress transactions but only reached `24,623` strict approved
    transactions, confirming material run-to-run variance and the same
    queue-drain/block-validation bottleneck.
  - The fresh post-cache sampled 20k profile at
    `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152126` confirms
    the next target has moved from queue gossip encoding to
    `validate_block_for_voting` / `validate_and_record_transactions` /
    `TxOverlay::apply_with_chunk`, incoming transaction-gossip Norito decode,
    and the remaining `AcceptedTransaction::signed_encoded_len` serialization
    fallback.
  - The targeted post-cache tuning pass at
    `dist/izanami-prebuilt-20k-postcache-tuned-120s-20260501-165947` improved
    strict approved transactions over the cachepass repeat to `28,790`, but
    still failed the committed 20k target and accepted fewer ingress
    submissions. The matching sampled profile at
    `dist/izanami-profile-20k-postcache-tuned-sampled-30s-20260501-165811`
    confirms `Queue::encode_gossip_payload`, `TxOverlay::byte_size`, and
    `external_entrypoints_cloned` are absent from current peer samples.
  - The further conservative cache pass is focused-validation green as of
    2026-05-01: prepared transaction metadata is reused through block
    validation/execution recording, all-external block validation keeps
    borrowing the entrypoint slice, signed/external entrypoint encoded-length
    coverage avoids the residual Norito fallback for representative shapes, and
    gossip transaction decode now uses the shared cached payload helper.
  - The clean release 4-peer no-fault prebuilt `20k TPS` / `120s` rerun at
    `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`
    exited `0`, accepted `54,574` ingress transactions, and reached `28,710`
    strict approved transactions at strict height `9`. This is consistent with
    the prior tuned gates and still misses the committed 20k target, with no
    validation rejects, view changes, or RBC pressure.
  - A later requested same-shape rerun at
    `dist/izanami-prebuilt-20k-conservative-cache-rerun2-120s-20260501-144548`
    exited `0` but ran under active debug `cargo test`/`rustc` contention. It
    accepted `52,070` ingress transactions and reached only `12,329` strict
    approved transactions at strict height `5`, with safety intact but `4`
    view-change installs and missing-block recovery activity. Treat this as
    contended evidence only, not a replacement for the clean baseline.
  - The matching requested contended sampled profile at
    `dist/izanami-profile-20k-conservative-cache-rerun2-sampled-30s-20260501-145104`
    exited `0` with valid samples for the load driver and all four peers;
    `sample_status=1` only because the sampler also targeted the bash wrapper
    and one transient process. It accepted `52,817` ingress transactions and
    reached `4,137` strict approved transactions at strict height `3`. The
    bottleneck shape matches the previous conservative-cache profiles: Torii
    admission crypto/public-key parsing, canonical signed-byte construction,
    residual dynamic `InstructionBox` framing, gossip materialization/decode,
    and overlay execution/cloning. Treat it as contended bottleneck evidence,
    not a clean latency baseline.
  - The earlier conservative-cache sampled 20k profile at
    `dist/izanami-profile-20k-conservative-cache-parallel-sampled-30s-20260501-181025`
    confirms the previous removals are still absent
    (`Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`,
    `external_entrypoints_cloned=0`) and moves the next bottleneck set to
    Torii ingress signature/public-key work, canonical signed-byte construction
    in `AcceptedTransaction::from_external_with_hot_cache`, exact-length
    `InstructionBox` payload framing, gossip materialization during admission,
    and remaining overlay instruction clones.
  - The broader 20k bottleneck pass is focused-validation green as of
    2026-05-01. Lazy transaction-gossip materialization now preserves cached
    framed entrypoint bytes and skips semantic decode before route, plane, and
    known-duplicate filters; route-valid single-key Ed25519 gossip candidates
    use deterministic batch precheck through the existing signature-batch
    setting; overlay apply goes through the crate-private borrowed adapter while
    custom executors keep the owned path. The profile at
    `dist/izanami-profile-20k-postcache-tuned-bottleneck-30s-20260501-171955`
    is pre-broader-pass evidence; the fresh reruns are
    `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734` and
    `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`.
    The 120s gate kept final approved transactions flat against the previous
    gate (`28740` vs `28710`) but accepted fewer ingress submissions
    (`52291` vs `54574`), so treat the pass as bottleneck reshaping rather than
    a confirmed throughput win.
  - The fixed-runner follow-up sampled profile at
    `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`
    completed with `sample_status=0` and sampled the actual Izanami runner plus
    all observed peer processes. It classifies the next bottlenecks as
    direct-peer Ed25519/curve25519 verification math first, then allocation /
    `memmove` and Norito compact/decode work, with ZK/BLS math and hashing as
    secondary costs. Queue mechanics and borrowed overlay apply are not primary
    CPU bottlenecks in that sample.
  - The latest clean rebuilt release 4-peer no-fault prebuilt `20k TPS` /
    `120s` gate is
    `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`;
    it exited `0`, accepted `47,566` ingress transactions, and reached
    `20,499` strict approved transactions at strict height `7`. The contention
    snapshots only contain timestamps. Safety signals stayed clean, but the
    run still saturated the queue and ended with height skew `1` /
    approved-transaction skew `8,192`, so the 20k target remains open.
  - The latest fixed-runner sampled profile at
    `dist/izanami-profile-20k-rerun-release-sampled2-30s-20260501-211211`
    completed with `sample_status=0`, accepted `46,709` ingress transactions,
    and reached `4,125` strict approved transactions at strict height `3`.
    The current peer CPU stack is led by `iroha_zkp_halo2::poseidon` /
    `fastpq_isi::poseidon`, `memmove` and allocator paths, `sha2`/`blake2`
    hashing, Norito compact-length/decode/encode routines, and then
    `curve25519_dalek` / `ed25519_dalek` verification math. Direct ingress
    batch precheck remains visible but is not the dominant leaf in this sample;
    overlay clone and exact-length helpers are low-count residue.
  - The direct-ingress conservative cache and precheck slice is code-complete
    as of 2026-05-01: Torii signed transaction and batch submission now decode
    versioned signed payloads into a prepared core admission token and run
    deterministic single-Ed25519 batch precheck for eligible batch entries,
    reusing signed/entrypoint hashes, payload hash, exact signed length, and
    parsed single-Ed25519 key metadata without changing transaction wire/hash
    semantics, config knobs, dependencies, or `Cargo.lock`.
  - The exact-length `InstructionBox` cost is reduced without changing Norito
    wire: `encoded_len_exact` now counts the existing `(wire_id,
    framed_payload)` representation without re-framing the dynamic ISI payload.
  - The FASTPQ/Poseidon foreground pass is implemented: single-delta transfer
    transcript digests are finalized at block/witness drain instead of inside
    `Transfer::execute`, FASTPQ digest hashing streams bytes without a full
    preimage buffer, and decoded external entrypoint hashes now reuse the
    inbound versioned signed payload bytes.
  - The first FASTPQ BN254 Metal Poseidon batch path is implemented behind the
    existing `fastpq-gpu` feature and existing FASTPQ execution/poseidon modes.
    Later Metal and CUDA parity/performance closure evidence is recorded in
    `status.md`; this historical slice no longer carries open GPU validation
    work.
  - Carry the Norito sequence span planner through the remaining acceleration
    corridor: replace the length-prefixed helper's serial device parser with a
    tuned prefix-scan/chunked planner if profiling shows it is on the hot path,
    expand typed parallel sequence decode beyond the current hidden
    `parallel-decode` `Vec<T: Send>` path if profiling proves narrower
    transaction/admission/block-validation call sites need it, then rerun the
    30s sampled 20k profile and 120s gate with the target host's acceleration
    features.
  - The latest scalar release 4-peer no-fault prebuilt `20k TPS` / `120s` gate
    after the Norito span-planner pass is
    `dist/izanami-prebuilt-20k-rerun-release-norito-span-120s-20260502-015557`;
    it exited `0`, accepted `47,503` ingress transactions, reached
    strict/quorum height `10`, and approved `32,786` transactions. The latest
    matching scalar sampled profile is
    `dist/izanami-profile-20k-norito-span-sampled-30s-20260502-020217`; it
    shows Norito transaction/instruction codec as the current top active peer
    path, followed by Poseidon/Ed25519/Curve/hash work, Rayon proof/hash
    scheduling, allocation/copy churn, TLS/context lookup, and Torii admission
    queue routing. Use this artifact as the baseline before the next
    optimization pass.
  - Continue reducing Norito decode/allocation overhead on the direct and
    gossip admission corridors without changing wire bytes or canonical hashes.
    `InstructionBox::DecodeFromSlice` now uses the borrowed tuple parser
    directly and `ExecutionStep::DecodeFromSlice` now delegates its inner
    instruction list to `ConstVec<InstructionBox>`. `ConstVec<T>` slice decode
    now tries the scalar Norito sequence planner directly for non-`u8` elements
    before falling back to the canonical `Vec<T>` field path, removing the
    top-level archive/canonical-length pass from the hot instruction-vector
    route. `AcceptedTransaction` also now derives the cached signed frame and
    external entrypoint hash from one canonical signed payload in the hot-cache
    path, avoiding a second signed-transaction serialization. `SignedTransaction`
    and `TransactionPayload` slice decoders now walk AoS fields directly, and
    `Executable::Instructions` routes the instruction vector into the planned
    `ConstVec<InstructionBox>` decoder before falling back for other executable
    variants. A fresh WSL2 no-profiler validation run after this
    admission-decode pass is recorded in `status.md`:
    `dist/izanami-prebuilt-20k-admission-decode-unsampled-30s-20260506-020112`
    accepted/succeeded all `600,000` offered submissions, and
    `dist/izanami-prebuilt-20k-admission-decode-120s-20260506-020335`
    accepted/succeeded `2,379,055` submissions with no safety failures but only
    `20,553` strict-approved transactions. Treat these as fresh ingress/safety
    evidence, not a bottleneck profile: the host had neither `sample` nor
    `perf`, and the 2.4M prebuilt-buffer run consumed nearly all WSL2 memory.
    Individual instruction payload slice paths are now in place for `Log`,
    `RecordSccpMessage`, the canonical grouped instruction boxes
    (`RegisterBox`, `UnregisterBox`, `MintBox`, `BurnBox`, `TransferBox`,
    `SetKeyValueBox`, `RemoveKeyValueBox`, `GrantBox`, `RevokeBox`,
    `RwaInstructionBox`, `RepoInstructionBox`, and `SettlementInstructionBox`),
    transfer batches, account signatory/quorum changes, the stable core
    SetParameter/trigger/upgrade/custom ISIs, asset-definition
    alias/balance-policy instructions, asset transfer-control instructions,
    account alias binding/lease instructions, contract-alias instructions,
    account-recovery instructions, RAM-LFE program-policy instructions,
    hidden-identifier instructions, consensus-key lifecycle instructions,
    domain-endorsement instructions, verifying-key instructions, Offline
    note instructions, verified Nexus lane-relay/fee-budget instructions,
    native and anonymous asset escrow lifecycle instructions, Musubi
    package-registry instructions, smart-contract-code
    manifest/instance/bytecode instructions, the Space Directory manifest
    lifecycle instructions, SoraFS pin/capacity/replication/provider-owner plus
    pricing/credit instructions, oracle feed/observation/dispute/governance/
    Twitter binding instructions, bridge proof/receipt/SCCP instructions,
    Ministry citizen-agenda proposal submission, social Twitter reward/escrow
    instructions, registered public-lane staking instructions, invalid-
    instruction placeholders, SoraNet VPN lease open/settle/refund instructions,
    runtime-upgrade ISIs, SNS name ISIs, ZK proof/confidential/election ISIs,
    Kaigi session/relay ISIs, governance proposal/ballot/citizen ISIs,
    Soracloud service lifecycle, host/placement, agent, model/training,
    rollout, runtime-state, mailbox, and receipt ISIs, and Nexus
    emergency-validator override ISIs via an opt-in registry constructor. The
    default registry no longer exposes direct grouped generic wire forms such
    as `Register<Domain>`, concrete mint/burn/transfer variants,
    `Grant<Permission, Account>`, `RepoIsi`, or `DvpIsi`; canonical clients use
    the grouped boxes. Remaining targets are the standalone entries that still
    use the generic constructor, broader allocation/memmove churn around
    transaction admission material, and a sampled 30s profile plus clean 120s
    gate on a profiler-equipped host after the next scalar admission-decode
    pass.
  - The FASTPQ BN254 Metal validation was completed in later accelerator
    closure passes recorded in `status.md`; keep new profiling here focused on
    the remaining scalar admission/decode and Ed25519 authority costs.
  - Keep an Ed25519 parsed-public-key/signature verification cache or a
    deterministic batch corridor for the Torii/direct-ingress single-key
    Ed25519 authority path as the next crypto follow-up after the
    Poseidon/source-attribution and Norito allocation work. Gossip-side
    deterministic Ed25519 batch precheck is already implemented.
  - Rerun 4-peer no-fault prebuilt `5k` and `10k TPS` rows as needed to locate
    the new knee after the conservative cache pass.
  - The targeted built-in overlay path now avoids the full `InstructionBox`
    clone before `Executor::Initial` dispatch; user-provided executors still
    use the owned fallback. Keep the broader borrowed-instruction execution
    rewrite separate unless a later post-crypto/decode profile again shows
    `Transfer::clone`, `WorldTransaction::apply`, or the concrete instruction
    handler clones as active costs.
  - Treat RBC authoritative-payload delays as symptoms of slow validation and
    materialization unless a later profile shows DA/RBC storage pressure,
    missing `BlockCreated`, or QC payload-missing counters.
  - Move FASTPQ worker budgeting and deterministic hardware-accelerated crypto
    investigation into the next tuning branch if the post-deferral profile
    still shows background prover Poseidon work competing with consensus. Keep
    the full borrowed-`Execute` executor API rewrite separate unless a later
    profile makes overlay execution dominant again.
- Turn the proposal-gap / queue-pressure investigation into a reproducible measurement pass.
  - Rerun the 7-peer load that previously advanced slowly or stalled under backlog.
  - Sample `/v1/sumeragi/status`, pending-block / commit-inflight metrics, and queue depths throughout the run.
  - Use a load generator that can actually sustain the target rate before changing worker/backlog tuning again.
- Rebaseline sorted asset-definition query performance.
  - Rerun `snapshot_ephemeral_sorted_asset_defs_first_batch` and `snapshot_stored_sorted_asset_defs_first_batch` on an isolated host.
  - If stored-mode still regresses, tune `stored_sorted_fast_start_params` / first-batch thresholds and keep the matching query tests aligned.
  - Restore a green `cargo test -p iroha_core` baseline for the query-performance branch after any tuning.

## Targeted follow-ups

- Migrate the remaining operator VPN workflows to submit the Torii-returned
  native `OpenVpnLeaseEscrow` and `SettleVpnLease` transactions, then retire
  the legacy in-memory receipt endpoint after a public relay/helper/Torii
  canary.
- Broaden Kura replay determinism beyond the current unit and consensus
  integration corridor.
  - Sidecar recovery semantics are now aligned with the memory-only WSV model:
    commit manifests and WSV checkpoints are optional verification metadata,
    while intact Kura blocks remain the recovery source of truth. Remaining
    work should prove replay equivalence from blocks, not make sidecars
    mandatory.
  - Commit-worker coverage now proves that injected WSV checkpoint and commit
    manifest write failures after state commit are reported as sidecar
    warnings, not ledger data loss or commit rollback.
  - The broad `integration_tests --test consensus_and_da` target is green after
    the memory-only WSV sidecar changes, including DA restart/rehydration and
    the mode-cutover and vote-QC regressions exposed by the first workspace
    rerun.
  - Add a multi-block replay fixture that replays committed blocks into a fresh
    state and compares canonical WSV snapshot bytes against the originally
    committed WSV.
  - The real 4-peer restart integration test now commits route-sensitive
    asset, account, alias, and domain-owned state, removes optional sidecar
    metadata, rebuilds from Kura, and compares the restarted peers' rebuilt
    query surface.
  - Keep the fixture on the replay-specific validation entrypoint so legacy
    blocks without embedded context remain covered separately from newly
    proposed blocks.
  - Add golden old-block Norito fixtures produced by a pre-context binary,
    rather than only synthesized absent-field decode tests.
  - Profile the post-commit canonical WSV checkpoint hash under sustained load
    and either record the accepted overhead or replace it with a cheaper
    committed state-root path.
  - If operators need a network-authenticated replay proof, promote the WSV root
    from a local Kura sidecar into block-committed or certificate-bound metadata.
- Broaden alias auto-renew mutation coverage beyond the focused onboarding grant.
  - Add an integration test proving a user-signed enable/disable update can mutate the subscription NFT created by onboarding.
  - If a non-onboarding mutation path still hits `Can't modify NFT from domain owned by another account`, capture the exact submitter, NFT id, and permission token shape before changing the permission model again.
- Add a live multi-peer multisig test for previously unregistered signatories.
  - Start from the existing materialization coverage in `integration_tests/tests/multisig.rs`.
  - Add a case where a signatory is materialized by registration and then successfully authors `MultisigPropose` / `MultisigApprove` on the network.
  - Assert transaction-authority shape and final instruction execution, not only account materialization.
- Extend and burn down the translation metadata audit backlog.
  - Refresh the translated `docs/formal/sumeragi/README.*.md` bodies after the
    English-only frontier formal and 2026-05-03 process-hardening updates so
    `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --require-current`
    can be restored for formal docs.
  - The Sumeragi frontier model, process invariants, mutation suite, TLC
    cross-check, and longer nightly bound are wired, and CI now publishes a JSON
    metadata report for the stale translated formal READMEs; the remaining
    formal-doc task is translation refresh only.
  - Clean the existing `docs/source` and `docs/portal` metadata debt, including files missing `source_hash` and `translation_last_reviewed`, before adding those trees to the CI gate.
  - Refresh only the files the checker flags, then record the clean audit command in `status.md`.
- Add a recorded capture gate for the default `sora-temple` petal styles.
  - Use `petal score-styles` with a published style set, profile, seed, and minimum success ratio.
  - Record the JSON baseline in `status.md` and keep the default style honest under aggressive capture.
  - Only add a stronger default variant if the current `sora-temple` family cannot meet the agreed gate.
