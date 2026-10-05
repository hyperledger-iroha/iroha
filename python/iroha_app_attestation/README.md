# Shared Iroha app attestation

Bounded server-side verifiers for platform evidence about app-owned hardware
keys: Apple App Attest attestations, assertions and fraud receipts, Android
KeyMint key attestation, live Google revocation, and Play Integrity Standard
tokens. The package depends only on the Python standard library and a
caller-selected OpenSSL 3 executable.

Nothing here issues a credential or grants monetary authority. A verified
result records what the platform attested. The caller selects every scope value
(challenge, release, profile, lane, app identity and roots) from its own
authenticated policy, never from the evidence, and composes the separate checks
below itself. The credential issuer and the enrollment and renewal routes of the
wallet design (`specs/kagemusha_single_design_proposal.md` §2.2) are not
implemented in this package yet.

## Modules

- `attestation.py` checks raw evidence against an independently selected
  `Selection` (client nonce, server nonce, release ID, hardware profile ID,
  attested key ID and lane ID). It parses bounded CBOR and DER and verifies one
  exact leaf-to-root X.509 path to a pinned root at a trusted time
  (`verify_pinned_chain`).
  - Apple (`verify_apple_raw`): the App Attest object must match the pinned App
    ID and environment, attested key, credential ID and the nonce derived from
    the selection. The nonce extension must use Apple's tagged
    `SEQUENCE { [1] EXPLICIT OCTET STRING }` form. Signed validation-category and
    bundle-version extensions are parsed when present; only categories 2
    (TestFlight), 3 (development), 4 (App Store) and 5 (enterprise/ad hoc) are
    admitted.
  - Apple assertions (`verify_apple_assertion`): exact server-selected client
    data, the stored attested key and App ID, the ES256 signature over
    `SHA256(authenticatorData || SHA256(clientData))`, and a counter strictly
    greater than the previous one. `DurableAppleAssertionCounterStore` keeps the
    counter in a full-sync SQLite transaction and consumes each client-data
    digest once across keys and restarts. Apple permits counter gaps; a counter
    does not prove that offline state cannot be rolled back.
  - Android (`verify_android_raw`, `verify_android_persistent_app_key_raw`):
    the chain envelope is `KMCA` + `01` + a certificate count and length-prefixed
    DER certificates, leaf first, 2 to 8 certificates, at most 128 KiB. The
    KeyDescription is taken from the extension-bearing certificate nearest the
    pinned root, and its public key must equal the leaf key, so an added leaf
    cannot substitute a software key. Evidence must match the package, signing
    certificate, challenge and locked verified boot, with a hardware-generated
    P-256 SIGN/SHA-256 key at matching attestation and KeyMint security levels
    from the caller's allowlist; usage-limited keys are rejected. KeyMint fixes
    its challenge at key generation, so the selection carries an all-zero
    attested key ID. Google's 2016 factory root is accepted after its expiry
    only when the root was valid before its pinned expiry and every other
    certificate is valid at the trusted time. Revocation is a separate check.
  - `android_patch_policy_met` computes the evidence-record fact
    `PATCH_POLICY_MET` (bit 4, `specs/kagemusha_wallet_wire_v1.md` §3.1) from
    the hardware-enforced OS, vendor and boot patch levels against a YYYYMM
    floor. An unmet floor is a recorded fact, not a rejection.
- `apple_receipt.py` verifies the App Attest CMS fraud receipt against a pinned
  Apple Root CA - G3 and binds it to the exact attested credential certificate.
- `revocation.py` fetches Google's attestation-status list over HTTPS without
  redirects or a proxy on every check and rejects a listed serial anywhere in an
  already verified Google-root chain.
- `play_integrity.py` decodes opaque Standard tokens at Google's fixed endpoint
  and checks the request hash, package, version, app-signing certificate,
  freshness and the recognition, licensing and device verdicts the selected
  policy requires. Testing responses are rejected. Only the decoder's HTTP 400
  rejects a token.
- `google_oauth.py` obtains the decoder's OAuth access token from an
  owner-only inherited credential descriptor and a public decoder-policy
  original. `openssl_private_rsa.py` performs the RS256 signature in-process
  through the loaded OpenSSL 3 library after checking the Root-owned module
  originals, so no private key reaches argv, the environment, a pipe or a
  temporary file.
- `native_time_interval.py` holds a bounded trusted-time interval.

A live dependency outage (revocation list, Play Integrity decoder or OAuth
token) raises `VerificationUnavailable`. It is still an `AttestationRejected`,
so generic handlers fail closed, but callers report it as retryable rather than
as rejected evidence.

These checks follow [Apple's App Attest validation steps](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server),
[Android's KeyMint attestation schema](https://source.android.com/docs/security/features/keystore/attestation)
and [Google's attestation revocation policy](https://developer.android.com/privacy-and-security/security-key-attestation).

## Tests

Run the package tests with the selected Python/OpenSSL runtime:

```sh
PYTHONPATH=src PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -v
```

Synthetic fixtures, Apple's published sample attestation and scripted Google
replies exercise the parsers, cryptographic boundaries and durable stores. They
do not establish physical-device qualification.
