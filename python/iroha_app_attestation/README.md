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
implemented as a serving issuer in this package yet. `wallet_enrollment.py` now composes
the retained real verifiers for current E1 enrollment; Native signing/HTTP/DATA ownership
and the deployed private worker remain required before issuance is enabled.

## Current E1 entry points

`WalletEnrollmentScope` consumes the exact current 194-byte Native-selected challenge
transcript and separately validated payment public key. It derives the model's E1 challenge
and payment-key binding digests, with fixed current hash vectors in `test_wallet_enrollment`.
`verify_android_wallet_payment_key_raw` passes the challenge unchanged to KeyMint validation;
`verify_apple_wallet_attestation_raw` and `verify_apple_wallet_enrollment_assertion` pass the
current H values unchanged as App Attest `clientDataHash`. No old `Selection` is reconstructed
by these current consumers and no extra SHA256 is applied to an already hashed H value.

`verify_android_wallet_enrollment` independently composes actual pinned KeyMint chain/app/key
checks, live Google revocation and opaque-token Google Play Integrity decoding. Its exact
ordered evidence items are the original leaf-first DER chain followed by the original server
Google decoder response, recording enrollment-time PI in the evidence digest. This item-order
extension requires matching wire documentation before publication. It creates no periodic PI
lease. The caller supplies a configured Google-root policy; other roots need a real separate
revocation adapter. `verify_apple_wallet_enrollment` verifies production App Attest plus a fresh
payment-key binding assertion using the existing durable counter store. It never fabricates
Secure Enclave attestation, OS/patch/jailbreak fields or hardware monetary-state authority.

The private Native issuer must select policy and existing account ownership, retain/consume
the exact E1 and originals with its audited DATA journal, and sign a compact credential using
its actual root-delegated Enrollment-role P256 key. Python request input exposes no signing
authority. An interrupted external attempt retrieves its original; it never resets a counter
or repeats a consumed assertion. `wallet_enrollment_worker.py` implements the current private
verification/recovery channel. Before exposing E1, Native retains the worker's actual journal
incarnation, selects its immutable preparation and durably retains the worker acknowledgement.
Complete or Recover can atomically claim that prepared row once. A delayed first claim uses
fresh trusted Native dispatch time without changing its original request. Before claiming,
the Linux worker samples OS realtime and retains `CLOCK_BOOTTIME`. Fresh readings include
suspend and are checked around KeyMint, revocation, OAuth/Google and Apple verification,
and before the durable result commits. Realtime regression or missing boottime is unavailable;
there is no suspend-excluding clock fallback. Google evidence age is checked again after
decode. The evidence timestamp retains the selected Core dispatch time; the local clock
only narrows whether the operation may still finish.

Once claimed, an unknown result stays `outcome_unknown`; no action repeats external
verification. Inspect reads an exact retained result without claiming a prepared row,
even while E1 is live. An unclaimed Inspect returns `unavailable`; an already claimed row
without a result returns `outcome_unknown`. The issuer uses Inspect for passive recovery
after E1 expiry and requires fresh bank eligibility before any live Complete/Recover that
could claim a row. Missing prepared custody remains `unavailable`, never a definitive
evidence rejection or permission to recreate a row. Native prohibits Prepare after E5
selection, including during recovery.
The journal retains exact request/result originals and their configuration pin. Apple assertion
counter/challenge consumption and the recoverable evidence result commit in one FULL-synchronous
transaction. Changed retained originals are rejected. The inherited configuration, crypto
original and storage identities are rechecked before exposure. These components do not alone
enable issuance: the genuine Native issuer, audited DATA and authenticated installed runtime
must admit and retain every original.

The worker opens only an already initialized E1 journal. Its separate installation operation
`E1CounterStore.initialize` exclusively creates a durable generation original before creating
the database, and binds that generation in the database's exact first-release schema. Serving
startup never initializes missing files or repairs missing tables. Database, generation or
schema loss remains unavailable; interrupted initialization retains its originals for explicit
operator reconciliation. SQLite connections use existing-only `mode=rw`, so a missing database
between the custody check and open cannot become an empty replacement. Held descriptor/path
checks reject substitution, and the counter/attempt tables and journal generation retain their original contents
across restart. These checks do not detect privileged rollback of the entire store. Native
installation/runtime admission and service dispatch still require integration.

`tools/build_wallet_e1_verifier_zipapp.py` packages an explicit current source inventory,
including `wallet_policy.py`, into deterministic unsigned bytes. It does not authenticate a
runtime. The private Linux owner uses OAuth13, archive15, Python16, protected directory17,
configuration20 and OpenSSL21; requests and replies are length-framed on private standard
input/output with schema `iroha.kagemusha.wallet-e1-verifier.v1`. It has no listener or
issuer-key input. The separate existing Ed25519 account ownership message remains Native's.

The exact configuration schema is `iroha.kagemusha.wallet-e1-verifier-config.v1`, version 1.
Its fields are `platform`, `app_policy_hex`, `enrollment_policy_hex`, `openssl_path`,
`openssl_sha256`, `store_directory` and `policy`, in addition to `schema` and `version`.
Both platform policy objects contain `scheme_id_hex`, `asset_digest_hex`, `root_base64`,
`root_sha256`, `regulatory_policy` (exactly `permitted_controls`, `blacklist_max_age_ms`,
`time_anchor_max_response_ms`), `challenge_lifetime_ms` and `attestation_lease_lifetime_ms`.
Apple adds `app_id`. Android adds `package_name`, `package_version`,
`app_certificate_sha256`, `security_levels` (`[1]`, `[2]` or `[1,2]`),
`patch_floor_yyyymm`, `google_policy_base64`, `google_policy_sha256`,
`maximum_evidence_age_ms`, `require_play_recognized`, `require_licensed` and
`minimum_device_integrity`. Unknown, duplicate, mistyped and noncanonical fields are rejected.
The worker rederives current Model app/enrollment policy digests from this projection and
matches Native's exact pins and E1; it checks the selected lifetime without refreshing it.
Rust's `issuer_worker::VerifierConfigurationV1` now constructs this exact projection from
the selected typed Model policies, pinned root and decoder originals and private runtime
locations. It retains the exact configuration and derives subsequent requests under the
same policy/configuration pin. Construction checks consistency only; the process owner
must still authenticate operator approval, installed runtime and descriptor custody.
The separate governed Google decoder original has its own pin and the same selected app.
OAuth accepts only the inherited Root-owned credential original and preserves the actual
loaded OpenSSL/TLS custody checks. Enrollment-time Google verification creates no offline
payment prerequisite or periodic Integrity lease.

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

From this package directory, use Python 3.10+ and the selected OpenSSL 3 on `PATH`.
Build the unsigned auth-only archive fixture before full discovery:

```sh
mkdir -p ../../target/qualification
attestation_test_dir="$(mktemp -d "$(cd ../../target/qualification && pwd)/app-attestation.XXXXXX")"
python3 -B tools/build_retail_auth_verifier_zipapp.py \
  --generic-package "$PWD" --auth-package "$PWD" \
  --output "$attestation_test_dir/iroha-retail-auth-verifier.pyz" \
  --inventory "$attestation_test_dir/archive-inventory.json"
BPNG_AUTH_TEST_BUILDER="$PWD/tools/build_retail_auth_verifier_zipapp.py" \
BPNG_AUTH_TEST_ARCHIVE="$attestation_test_dir/iroha-retail-auth-verifier.pyz" \
BPNG_AUTH_TEST_WORKER="$PWD/src/iroha_app_attestation/retail_auth_worker.py" \
TMPDIR="$attestation_test_dir" PYTHONPATH=src PYTHONDONTWRITEBYTECODE=1 \
  python3 -B -m unittest discover -s tests -v
```

Synthetic fixtures, Apple's published sample attestation and scripted Google
replies exercise the parsers, cryptographic boundaries and durable stores. They
do not establish physical-device qualification.
