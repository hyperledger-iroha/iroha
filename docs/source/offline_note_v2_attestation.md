# Offline Note V2 Device Attestation

Offline Note V2 supports two device-attestation admission flows. Deployments can
use both at the same time:

- Middleware receipt flow: a centralized or operator-run verifier checks Apple
  App Attest or Android KeyMint evidence off-chain, signs a short JSON receipt,
  and Torii issues the Offline Note key certificate after validating that
  receipt.
- On-chain registration flow: the wallet submits
  `RegisterOfflineDeviceAttestation` with raw platform evidence, and every node
  verifies the platform evidence deterministically during chain execution.

The middleware flow is operationally convenient for local device debugging and
for platforms whose verification dependencies should stay outside consensus. The
on-chain flow is the trust-minimized path for offline use cases such as
spend-again-offline transfers, because consensus records replay markers for the
certificate payload, challenge, report, and evidence.

## Middleware Receipt Contract

Torii does not verify the raw Apple or Android attestation blob in this flow. It
trusts only `device_binding.attestation_receipt`, which must be signed by
`attestation_verifier_public_key` over the canonical unsigned JSON receipt with
`signature_base64` removed.

Offline V2 issuer POST bodies use in-body app authentication: `account_id`,
`timestamp_ms`, `nonce`, and exactly one proof field, either
`signature_base64` or `witness_base64`. The proof field must be a non-empty
exact string; present null, non-string, empty, or leading/trailing-whitespace
values are rejected before canonical request verification. `account_id` and
`nonce` are also exact protocol strings in the body-auth prelude. The canonical
signed body removes only the top-level proof fields, so nested fields with the
same names remain signed business data.

Issuer request protocol strings are exact when present: `account_id`,
`device_id`, `offline_public_key`, `asset_definition_id`, `operation_id`,
`existing_lineage_id`, and note-issue `lineage_id` reject empty values and
leading or trailing whitespace instead of being trimmed into a different
canonical body. The optional `X-Device-Id` HTTP header follows the same exact
rule when present, and must match the exact JSON `device_id`.

The signed receipt version is `1` and contains these fields:

- `version`: `1`.
- `platform`: platform label. First-release receipts accept the legacy
  middleware iOS profile `ios-app-attest`, the canonical on-chain iOS profile
  `ios-appattest`, and the canonical Android profile `android-keymint`.
- `account_id`: exact account string from the request.
- `device_id`: exact device id from the request.
- `offline_public_key_base64`: canonical standard base64 for the 32-byte Offline
  note public key. It must match request `offline_public_key`.
- `assertion_public_key_base64`: canonical standard base64 for the hardware
  assertion public key. It must match any request `assertion_public_key`,
  `app_attest_public_key_base64`, or `device_public_key` field when present,
  and it must encode a valid uncompressed SEC1 P-256 point.
- `assertion_scheme`: hardware assertion scheme.
- `assertion_key_algorithm`: assertion key algorithm.
- `assertion_usage_count_limit`: absent for both iOS App Attest profiles; for
  Android KeyMint it must be present as unsigned integer `1`.
- `attestation_key_id`: verifier-scoped platform attestation key id.
- `hardware_one_use`: must be `true`.
- `attestation_report_hash_hex`: lowercase or uppercase hex accepted by hash
  comparison, but it must encode exactly 32 bytes.
- `issued_at_ms`: Unix milliseconds. It must not be in the future.
- `expires_at_ms`: Unix milliseconds. It must be greater than the current Torii
  time and greater than `issued_at_ms`.
- `signature_base64`: Iroha signature bytes over the unsigned receipt JSON,
  encoded with canonical standard base64. It must decode to exactly 64 bytes.

No other signed receipt fields are accepted. Unknown fields are rejected before
the receipt is used, even when they are covered by an otherwise valid verifier
signature. Signed receipt string fields are exact protocol values: Torii rejects
empty strings and strings with leading or trailing whitespace instead of
normalizing them before comparison or key-certificate issuance.

If `device_binding.attestation_report_base64` is present, Torii requires it to
be an exact, non-empty base64 string, decodes it, and requires
`sha256(report_bytes)` to equal `attestation_report_hash_hex`. If the request
supplies `assertion_public_key`, `app_attest_public_key_base64`,
`device_public_key`, `platform`, `assertion_scheme`, or
`assertion_key_algorithm` inside `device_binding`, those string values must be
exact and must match the signed receipt. The Android KeyMint profile requires
both the signed receipt and `device_binding` to carry
`assertion_usage_count_limit = 1`; both iOS App Attest profiles must omit it.

The signed receipt must also use one of these profile triples:

```text
platform        assertion_scheme                              assertion_key_algorithm
ios-app-attest  apple-app-attest-v1                           ecdsa-p256-sha256
ios-appattest   apple-appattest-counter-v1                    app-attest-p256
android-keymint android-keymint-ecdsa-p256-usage-limit-v1     ecdsa-p256-sha256
```

For both iOS App Attest profiles, `assertion_usage_count_limit` must be absent
from the signed receipt and `device_binding`. For the Android KeyMint profile,
it must be present in both places and equal to `1`.

The Swift, Kotlin/JVM, and Java Android SDK registration constructors enforce
the canonical on-chain profiles before encoding
`RegisterOfflineDeviceAttestation`: `ios-appattest` uses no usage limit, and
`android-keymint` uses `assertion_usage_count_limit = 1`. They reject blank
`key_id` and `device_id` values before encoding, and iOS App Attest
registrations additionally require `key_id` to be canonical standard base64
credential bytes. Their key-certificate and key-certificate-payload models also
enforce non-empty attestation identities and the supported certificate profile
table above, including valid uncompressed SEC1 P-256 `assertion_public_key`
bytes. They can still read legacy middleware-issued iOS certificates through
the explicit `ios-app-attest` profile, but reject arbitrary hardware profile
names, profile splices, off-curve assertion keys, and unsupported usage-limit
values.

Torii also applies the same signed profile table when parsing a redemption
sender key certificate. This preserves legacy middleware iOS certificates while
rejecting profile-spliced certificates, iOS certificates with usage limits, and
Android certificates that omit usage limit `1`. Structured JSON redemption
payloads are field-strict: the `norito_base64` wrapper, the structured
redemption object, nested sender key certificates, and nested recursive proof
objects all reject unknown keys before interpretation. Compatibility aliases
remain accepted: `key_certificate` for `sender_key_certificate`,
`verifier_key_name` for `verifier_key_id`, `public_inputs_hash` for
`public_inputs_hash_hex`, and the SDK-emitted redemption identity fields
`recipient_account_id` and `asset_definition_id`. When present, those nested
identity fields must match the authenticated top-level request. Recursive proof
backend aliases `verifier_key_backend` and `proof_backend` are accepted
alongside `backend` only when all present backend fields carry the same exact
value. Verifier-key and public-input alias pairs remain mutually exclusive in a
single JSON object. The sender key certificate field set includes the Torii
issue-response envelope metadata (`issued_at_ms`, `expires_at_ms`,
`app_attest_public_key_base64`, iOS app metadata, and
`issuer_signature_payload_base64`) so wallets may reuse the returned
`key_certificate` object directly as a redemption `sender_key_certificate`.
Structured redemption proof material is exact-string parsed: `norito_base64`,
note commitment hashes, input nullifier hashes, recursive proof verifier key
ids, recursive public-input hashes, and recursive proof base64 all reject
leading or trailing whitespace instead of trimming before interpretation.
Torii's signed `lineage_state` and nested `authorization` objects are likewise
field-strict when clients present them back to the issuer; the authorization
`device_binding` remains an opaque signed sub-object for platform-specific
attestation details. Signed lineage-state and authorization string fields are
also exact protocol values, and the known authorization `device_binding`
identity fields (`device_id` and `offline_public_key`) reject leading or
trailing whitespace when present. Receipt, lineage-state, and authorization
JSON signatures must decode from canonical standard base64 to exactly 64 bytes
before signature verification.

Every Offline Note V2 key certificate accepted for redemption must also carry
non-empty `platform`, `key_id`, `device_id`, `assertion_scheme`,
`assertion_key_algorithm`, and `assertion_public_key` values, regardless of
whether the certificate came from middleware or on-chain registration. Torii and
chain admission both reject profile-bound redemption certificates whose
`assertion_public_key` is not a valid uncompressed SEC1 P-256 point or whose
platform/scheme/algorithm/usage-limit tuple is outside the supported profile
table above. Structured JSON key certificates use the same exact string rule as
signed receipts: certificate identity, profile, key-material, and issuer
signature fields are rejected if they need leading or trailing whitespace to be
trimmed before interpretation.

The receipt is intentionally not a chain object. It is accepted only by the
Torii Offline V2 issuer before that issuer signs the Offline Note key
certificate. Torii does not mount `/v1/attestation/issue`; that endpoint name is
reserved for an operator-run middleware service that owns the verifier private
key and verifies raw platform evidence outside the node process.

## On-Chain Registration Contract

The chain instruction is:

```text
iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation
```

Its payload is an `OfflineDeviceAttestationRegistration` encoded with Norito.
The field order is the Rust data-model order:

```text
version
platform
key_id
device_id
account_id
asset_definition_id
ios_team_id
ios_bundle_id
ios_environment
android_package_name
android_signing_certificate_sha256
public_key
assertion_scheme
assertion_key_algorithm
assertion_public_key
assertion_usage_count_limit
one_use
challenge_hash
attestation_report_hash
attestation_report
evidence_hash
evidence
recent_block_height
recent_block_hash
expires_at_ms
```

`challenge_hash` is not arbitrary. It must equal
`Hash::new(norito::to_bytes(OfflineDeviceAttestationChallengePreimage))`, where
the preimage fields are:

```text
domain = "iroha:offline-note:device-attestation-challenge:v1"
version
platform
key_id
device_id
account_id
asset_definition_id
ios_team_id
ios_bundle_id
ios_environment
android_package_name
android_signing_certificate_sha256
public_key
assertion_scheme
assertion_key_algorithm
assertion_public_key
assertion_usage_count_limit
one_use
recent_block_height
recent_block_hash
expires_at_ms
```

The challenge preimage excludes `attestation_report_hash`,
`attestation_report`, `evidence_hash`, and `evidence`, because the platform must
receive the challenge before it creates the attestation report.
For pre-attestation drafts, the Swift, Kotlin/JVM, and Java Android SDK
constructors therefore allow the default empty `attestation_report` plus empty
`evidence` inputs and synthesize the deterministic empty-report evidence
envelope so callers can read `challengeHash()`/`canonicalChallengeHash()` before
platform evidence exists.

The registration must also satisfy:

- `attestation_report_hash == Hash::new(attestation_report)`.
- `evidence_hash == Hash::new(evidence)`.
- `evidence` is a deterministic envelope:
  `b"offline-device-attestation-evidence-v1" || attestation_report_hash_bytes`.
- `public_key` encodes exactly 32 bytes.
- `one_use == true`.
- The registration `platform`, `key_id`, `device_id`, `assertion_scheme`, and
  `assertion_key_algorithm` must be non-empty exact protocol strings. Optional
  app identity metadata (`ios_team_id`, `ios_bundle_id`, `ios_environment`, and
  `android_package_name`) also rejects empty or leading/trailing-whitespace
  values when present, and the resulting key certificate carries non-empty
  `assertion_public_key`.
- `assertion_public_key` is an uncompressed SEC1 P-256 public key and must parse
  as a valid curve point.
- iOS App Attest uses the canonical on-chain profile
  `ios-appattest` / `apple-appattest-counter-v1` / `app-attest-p256` and omits
  `assertion_usage_count_limit`. Its `key_id` is the canonical standard-base64
  App Attest credential id; consensus rejects non-canonical base64 aliases and
  requires the decoded bytes to match the authenticator-data credential id and
  the SHA-256 digest of the certificate assertion public key.
  The shared Offline Note V2 interop fixture uses canonical standard-base64
  iOS key identifiers so Swift, Kotlin/JVM, and Java Android SDK golden tests
  exercise the same byte-level contract.
- Android KeyMint uses the canonical on-chain profile
  `android-keymint` / `android-keymint-ecdsa-p256-usage-limit-v1` /
  `ecdsa-p256-sha256` and sets `assertion_usage_count_limit` to exactly `1`.
  Its `key_id` is the lowercase hex SHA-256 digest of `assertion_public_key`;
  consensus and SDK constructors reject uppercase spellings even when they
  encode the correct digest bytes.
- `recent_block_height` and `recent_block_hash` name a recent committed block.
- `expires_at_ms` is still valid at execution time.

For iOS App Attest, consensus verifies CBOR and COSE structure, credential-key
binding, app identity, Apple-rooted X.509 chain, and the nonce extension over
`sha256(authenticator_data || challenge_hash)`. The top-level App Attest CBOR
object and embedded COSE public-key CBOR must consume their full byte strings;
trailing CBOR values are rejected. The leaf certificate must contain exactly
one App Attest nonce extension; duplicate nonce extension OIDs are rejected. For
Android KeyMint, consensus verifies the CBOR certificate array, X.509 chain,
KeyMint extension, subject-key binding, attestation challenge, hardware-backed
security levels, one-use usage limit, Android package name, and
signing-certificate digest. The top-level Android certificate-array CBOR must
also consume the full report byte string, and the leaf certificate must contain
exactly one KeyMint attestation extension. KeyMint extension DER is parsed
canonically: non-minimal length encodings, non-canonical high-tag numbers, and
non-minimal INTEGER encodings are rejected. The Android
`attestationApplicationId` package and signing-digest SETs must also be sorted
by their DER element encodings.

## Policy and Root Rotation

`SetOfflineDeviceAttestationPolicy` stores the governed verifier policy used by
the on-chain path. It can publish trusted platform root DER bytes, certificate
DER SHA-256 revocations, and optional app allowlists. If no policy is stored,
nodes use the built-in first-release platform roots.

Policy allowlist strings are exact non-empty ASCII values with no leading or
trailing whitespace. iOS Team ID and environment matching is case-insensitive;
iOS bundle identifiers and Android package names match byte-for-byte.

Do not put middleware private keys on-chain. The middleware flow keeps the
receipt-signing key off-chain by design. The on-chain flow needs no middleware
secret; it relies on public platform roots, governed policy, and the raw
attestation evidence submitted in the transaction.
