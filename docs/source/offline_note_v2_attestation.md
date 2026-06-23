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

The signed receipt version is `1` and contains these fields:

- `version`: `1`.
- `platform`: platform label, for example `ios-app-attest`.
- `account_id`: exact account string from the request.
- `device_id`: exact device id from the request.
- `offline_public_key_base64`: canonical standard base64 for the 32-byte Offline
  note public key. It must match request `offline_public_key`.
- `assertion_public_key_base64`: canonical standard base64 for the hardware
  assertion public key. It must match any request `assertion_public_key`,
  `app_attest_public_key_base64`, or `device_public_key` field when present.
- `assertion_scheme`: hardware assertion scheme.
- `assertion_key_algorithm`: assertion key algorithm.
- `attestation_key_id`: verifier-scoped platform attestation key id.
- `hardware_one_use`: must be `true`.
- `attestation_report_hash_hex`: lowercase or uppercase hex accepted by hash
  comparison, but it must encode exactly 32 bytes.
- `issued_at_ms`: Unix milliseconds. It must not be in the future.
- `expires_at_ms`: Unix milliseconds. It must be greater than the current Torii
  time and greater than `issued_at_ms`.
- `signature_base64`: Iroha signature bytes over the unsigned receipt JSON,
  encoded with standard base64.

If `device_binding.attestation_report_base64` is present, Torii base64-decodes it
and requires `sha256(report_bytes)` to equal `attestation_report_hash_hex`. If
the request supplies `platform`, `assertion_scheme`, or
`assertion_key_algorithm` inside `device_binding`, those values must match the
signed receipt. If `assertion_usage_count_limit` is present, it must be exactly
`1`.

The receipt is intentionally not a chain object. It is accepted only by the
Torii Offline V2 issuer before that issuer signs the Offline Note key
certificate.

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

The registration must also satisfy:

- `attestation_report_hash == Hash::new(attestation_report)`.
- `evidence_hash == Hash::new(evidence)`.
- `public_key` encodes exactly 32 bytes.
- `one_use == true`.
- `assertion_usage_count_limit`, when present, is exactly `1`.
- `recent_block_height` and `recent_block_hash` name a recent committed block.
- `expires_at_ms` is still valid at execution time.

For iOS App Attest, consensus verifies CBOR and COSE structure, credential-key
binding, app identity, Apple-rooted X.509 chain, and the nonce extension over
`sha256(authenticator_data || challenge_hash)`. For Android KeyMint, consensus
verifies the CBOR certificate array, X.509 chain, KeyMint extension, subject-key
binding, attestation challenge, hardware-backed security levels, one-use usage
limit, Android package name, and signing-certificate digest.

## Policy and Root Rotation

`SetOfflineDeviceAttestationPolicy` stores the governed verifier policy used by
the on-chain path. It can publish trusted platform root DER bytes, certificate
DER SHA-256 revocations, and optional app allowlists. If no policy is stored,
nodes use the built-in first-release platform roots.

Do not put middleware private keys on-chain. The middleware flow keeps the
receipt-signing key off-chain by design. The on-chain flow needs no middleware
secret; it relies on public platform roots, governed policy, and the raw
attestation evidence submitted in the transaction.
