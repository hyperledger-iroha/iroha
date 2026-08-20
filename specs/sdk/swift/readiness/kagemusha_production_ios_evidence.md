# Kagemusha production iOS evidence contract

This contract is separate from the Taira testnet candidate lab. Testnet
`iroha.kagemusha.ios_device_lab.signed_evidence.v1` receipts remain useful for
offline physical-device performance testing, but production promotion does not
accept them.

Production evidence uses
`iroha.kagemusha.ios_device_lab.production_signed_evidence.v1` and an exact
canonical `iroha.kagemusha.ios.production_device_policy.v1` document. The
signed envelope binds the policy identifier and SHA-256, final release-manifest
digest, exact candidate raw-artifact inventory, measured Team ID/bundle/build,
the independently prepared capture app's canonical code-sign measurement, and
raw App Attest attestation and assertion objects. The capture-app measurement
contains its Team ID, bundle/build, application identifier, production
entitlement, executable SHA-256, and CDHash. Its canonical SHA-256 is included
in both one-time challenge schemas and the exact object is included in platform
evidence, preventing another executable signed for the same App ID from being
substituted after challenge issuance. The policy supplies the
production App ID, permitted validation categories and bundle versions, Apple
App Attest root DER certificates, revocations, X.509 validation profile, and
the `DCAppAttestService` Secure Enclave key profile.

`scripts/kagemusha_production_ios_evidence.py` currently verifies, without a
PATH-selected cryptographic tool:

- the outer Ed25519 evidence signature and exact raw benchmark tree;
- policy ID/hash and release-manifest bindings;
- measured code-sign identity against the production App ID policy;
- exact challenge bindings and distinct 32-byte nonces;
- App Attest CBOR shape, production AAGUID, credential ID, COSE P-256 key,
  App ID/RP hash, extension policy, and positive assertion counter. The
  attestation requires AT but does not require ED: Apple's published 2026
  production vector sets the flag byte to `0x40` while appending its extension
  map. The documented validation-category `UInt32` is accepted in that
  vector's exact four-byte little-endian CBOR byte-string form (or as a bounded
  CBOR unsigned integer), and the mandatory trailing map is parsed directly;
  assertion AT and all reserved flag bits are rejected; and
- the assertion's ECDSA-P256-SHA256 signature after the same low-S
  normalization used by the native bridge;
- strict bounded X.509 v3 parsing, P-256/P-384 issuer-key validation, the exact
  leaf-to-intermediate-to-policy-root signature path, CA/basic-constraints and
  key-usage rules, certificate validity at the receipt-bound evidence time,
  and the policy's static certificate-revocation digest set;
- the leaf P-256 key against the App Attest key ID and registered assertion
  key; and
- the unique `1.2.840.113635.100.8.2` extension in its exact
  `SEQUENCE/[1]/OCTET STRING` form, equal to
  `SHA-256(authData || SHA-256(attestation_client_data))`.

The CBOR parser bounds arrays to 1,024 items and maps to 64 entries before
decoding their children. The X.509 input is bounded to four 64 KiB
certificates, 64 extensions per certificate, supported single-octet DER tags,
canonical lengths, and P-256/P-384 algorithms before elliptic-curve work.
Malformed policy collections stop typed platform validation and return
diagnostics instead of reaching an exception path.

Current Apple App Attest receipt acceptance, risk, and replay state are online
facts, not facts a static root bundle can prove. The validator therefore also
requires a bounded canonical
`iroha.kagemusha.ios.app_attest_online_freshness_consumption_receipt.v1`
signed by an Ed25519 authority key distinct from the lab signer. The receipt
binds the exact signed-evidence digest, policy digest, catalog-selected release
manifest digest, platform-evidence digest, both canonical client challenges,
both App Attest objects, both embedded server nonces, the attestation and
assertion message nonces, key ID, ordered certificate digests, previous and
new assertion counters, issuance/consumption/expiry times, and a distinct
one-time consumption identifier. The new counter must equal authenticatorData
and strictly exceed the authority-attested prior counter. Receipt lifetime and
the authority's Apple-status check are each limited to five minutes.

The exact v1 field names retain `apple_revocation_status` and
`apple_revocation_checked_at_unix_ms`. For this profile, `good` has a narrower
and testable meaning: Apple's production `/v1/attestationData` endpoint
accepted the embedded App Attest receipt; the returned PKCS#7/CMS signature
and certificate path validate only to the digest-pinned Apple Root CA G3; the
signed App ID, attested P-256 key, `RECEIPT` type, creation/expiry times, and
risk metric match policy; and the static production policy did not list an
App Attest certificate digest as revoked. The endpoint is a receipt
refresh/risk-assessment service, not a general Apple PKI CRL or OCSP feed.
The `apple_revocation_checked_at_unix_ms` value records the authority's
verification/commit time; the independently signed CMS creation time is also
required to be within five minutes but is not mislabeled as the check time.

`scripts/kagemusha_app_attest_freshness_authority.py` implements the authority.
Its `issue` operation commits distinct 32-byte attestation/assertion nonces and
a one-time consumption ID to an owner-private SQLite WAL before returning the
artifact-bound phone request. `consume` validates the complete signed evidence,
extracts the exact `attStmt.receipt`, calls only
`https://data.appattest.apple.com/v1/attestationData` with bounded TLS/HTTP/body
time and size, validates the returned CMS receipt, and uses `BEGIN IMMEDIATE`
plus full-sync WAL durability to atomically advance the per-key assertion
counter and consume the challenge before Ed25519 signing. A committed unsigned
receipt is retained so a crash between commit and signing recovers the same
receipt without another Apple call. Evidence substitution, challenge replay,
counter rollback, concurrent consumption, stale CMS creation, excessive risk,
wrong App ID/key/type/root, unsafe state ancestors/SQLite sidecars, and blocking
JWT descriptors fail closed in the focused test suite.

The consumption receipt is immutable capture history, not a renewable status
object. A catalog may retain up to 16 releases for rollback/audit, so a later
promotion must not rewrite an older release or pretend its five-minute receipt
is current. Instead, the authority's `revalidate-catalog` operation validates
every historical envelope and original consumption receipt in full, refreshes
each embedded App Attest receipt with Apple, and emits one bounded
`iroha.kagemusha.ios.app_attest_catalog_revalidation_receipt.v1`. That receipt
binds a single-use nonzero promotion ID, the canonical ordered digest of every
release manifest/evidence/original-receipt triple, and a fresh Apple status,
refreshed-receipt digest, App Attest key ID, and risk metric for every release.
It expires within five minutes. The SQLite authority reserves the promotion ID
before signing; crash recovery can recover only the identical unexpired
payload, while catalog rebinding, status substitution, and revival after expiry
fail closed.

The request consumed by `revalidate-catalog` uses schema
`iroha.kagemusha.ios.app_attest_catalog_revalidation_request.v1`, carries the
same promotion ID, and lists canonical absolute paths to each signed envelope,
raw artifact root, original consumption receipt, and prepared capture-app
measurement. DeviceCheck credentials remain runtime-only. The ordinary
`validate_production_signed_evidence` entrypoint always rejects an expired
consumption receipt; only the dedicated authority/promotion historical
entrypoint omits that one current-time comparison, and its callers must then
require the separate current exact-catalog receipt.

DeviceCheck JWTs enter only through an owner-private bounded file or inherited
regular-file descriptor and are never stored or logged. Production operations
must still provision and rotate the independent Ed25519 authority key and
short-lived DeviceCheck JWT issuer. Once provisioned, a valid signed receipt is
the validator's success path; an absent, malformed, untrusted, stale, or
substituted receipt fails closed. A lab signature, certificate-byte presence,
self-reported time, receipt Boolean, or `app_attest_used:true` never satisfies
the external requirement.

The physical capture application must use `DCAppAttestService.generateKey`,
then `attestKey(_:clientDataHash:)` over the exact canonical attestation client
data, and finally `generateAssertion(_:clientDataHash:)` over the exact
canonical assertion client data. The physical-only
`IrohaSwift/KagemushaProductionAppAttestLab` target and
`scripts/run_kagemusha_production_app_attest_lab.sh` implement that sequence,
require the signed application to retain the exact scalar `production`
entitlement before installation, and require the embedded provisioning profile
to authorize that selection either as the scalar `production` value or Apple's
unique bounded `development`/`production` authorization array. Unknown,
duplicate, malformed, or development-only profile values fail closed. Device
selectors and profiles stay out of retained evidence. Its default
artifact-independent request is
explicit qualification material with `promotion_eligible:false`; promotion
requires first retaining an independently prepared exact signed app with
`--prepare-only`, then installing that same measured app from `--prepared-root`
with a caller-supplied exact release-artifact-bound challenge pair. A one-shot
build/capture is qualification-only and cannot consume a production request. The
capture checker copies that pair's exact `evaluated_at_unix_ms` into the
platform material, but even its release-bound standalone summary remains
`promotion_eligible:false` until this full validator authenticates the signed
raw tree, production envelope, and online freshness/consumption receipt. The existing
`KagemushaRequestAuthorizationPreparation.authorizeWithIosAppAttest` Swift API
already exercises the assertion API for transaction authorization, but the
candidate evidence lab remains offline and does not perform the production
key-attestation capture. No synthetic CBOR or software P-256 key is acceptable
outside tests.

The benchmark host and standalone capture target deliberately use the same
policy-bound App ID, `org.hyperledger.iroha.kagemusha.appattestlab`. They are
different measured executables: the release envelope retains the benchmark
host measurement in the raw tree and separately embeds the prepared capture
app's canonical executable/CDHash measurement. Sharing the App ID lets one
explicit production App Attest provisioning policy authenticate both phases
without pretending that App Attest attests an executable hash.

After release-bound capture produces canonical platform evidence,
`scripts/sign_kagemusha_production_ios_evidence.py` validates the raw tree,
production policy, Apple chain/objects, capture-app measurement, final release
manifest digest, and matching Ed25519 keys before signing. It validates a
private temporary envelope and publishes it through an atomic no-replace link,
so an existing operator artifact is never overwritten. The result remains
incomplete until the independent online authority consumes that exact
envelope and signs its freshness/replay receipt.

Promotion additionally requires the production policy through
`KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY`. The gate snapshots that
policy, the trusted lab-signing key, and both reviewed iOS validators before
validation. Multi-release promotion also requires
`KAGEMUSHA_V4_PROMOTION_ID` and the root-custodied receipt at
`KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT`; the receipt must
stay outside the immutable evidence root and bind the exact complete release
set observed by the gate. A stale original receipt alone, a fresh receipt for a
different promotion, an omitted historical release, or a substituted evidence
or consumption-receipt digest fails closed. The standalone production checker
additionally requires `--freshness-receipt`, `--trusted-freshness-key-id`, and
`--trusted-freshness-public-key`; the promotion gate must snapshot equivalent
root-custodied inputs before activation. It also requires the production
envelope's `release_manifest_sha256` to equal the enclosing catalog directory
name.
`scripts/build_kagemusha_production_ios_policy.py` provides the strict
no-replace constructor for this policy and validates every emitted byte through
the same production parser before publication; policy choice and root-custody
review remain operator decisions.

Mutation coverage lives in
`scripts/tests/check_kagemusha_candidate_ios_evidence_test.py` and includes
policy ID/hash substitution, malformed policy collection types, CBOR container
count overflow, control bytes, omitted code-sign binding, raw-tree loss,
prepared capture-app measurement/executable substitution, assertion-signature
tampering, RP-ID substitution, fake and expired
certificates, static revocation, nonce substitution, receipt-signature and
evidence-digest substitution, stale online revocation status, counter rollback,
one-time consumption-ID reuse, two time-separated catalog releases, catalog
substitution, stale catalog status, and cross-promotion replay. The repository's pinned Apple App
Attestation P-384 root and Apple's complete published 2026 attestation object
are real parser/signature/wire fixtures; synthetic certificate chains remain
test-only and are never production evidence. The primary vector proves the
real leaf/intermediate chain, `AT=1, ED=0` authData, little-endian category,
nonce extension, Root CA G3 CMS path, and raw numeric receipt attributes.
The repository does not contain a capture from the exact reviewed Kagemusha
application on the attached phone or live DeviceCheck/authority signing
credentials.
Production activation therefore still requires a physical App Attest capture
plus runtime provisioning and operational review of this authority deployment;
the implementation and adversarial offline protocol tests are repository-owned.
