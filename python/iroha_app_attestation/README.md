# Shared Iroha app attestation

BPNG, BOI and CBSI use this shared Iroha implementation. Product adapters carry
Native-selected requests and exact platform originals. The first release requires
no custom applet, OMAPI access rule or OEM provisioning for Pixel 6 enrollment.
Android generates a persistent nonexportable P256 SIGN/SHA256 key, prefers
StrongBox, and accepts TEE only under the authenticated hardware policy. Imported,
software and usage-limited keys are rejected. Apple uses genuine App Attest
attestation and possession assertions with a separately retained assertion counter.
The Native financial secret and logical financial index are independent of these
platform approval keys and counters.

The current enrollment challenge is exactly 451 body bytes and a 64-byte Core
Ed25519 signature. It binds account, network, lane, release, profile, suite, policy
and financial epochs, nonces, attempt and the separate financial commitment.
The first E371 possession field is SHA256 of the full model-owned challenge
signing message; it is not the stable enrollment ID or the hash of the signed
515-byte transport. The final `KOAC01` encoder request is exactly 896 bytes,
including the separate Ed25519 identity and the 65-byte governed issuer P256 point.
Raw platform admission uses a separate `KRAC01` request and 314-byte signed
admission, retained before possession and final credential admission. Both native
encoders sign the actual Rust model and emit its canonical Norito originals.

Enrollment credentials and integrity leases also require a purpose-specific
issuer P256 countersignature over the SHA256 of the complete canonical Ed25519
original. The issuer point comes from the actual signed hardware profile; the
Native encoder checks it against the held issuer seed before signing. Both
recursive Guard parities verify this equation and keep credential identifiers
private. This signature authenticates the software issuer's admission, while the
separate platform signature proves possession of the generated hardware key.

`ordinary_provider.py` composes actual preparation, platform, possession and
current-policy checks. `ordinary_issuance.py` commits the exact original Google
response and signing input before publishing an enrollment credential. Identical
lost-result recovery returns the retained original without another Google decode
or signing operation. `ordinary_service.py` owns the strict credential DTO;
`ordinary_service.py` also owns the separate raw-admission DTO;
`ordinary_raw_admission.py` binds its original evidence to the model signing input.

Periodic Android refresh is a separate challenge and lease. The Core-signed
challenge is 450 body bytes plus 64 signature bytes. Its attempt ID hashes the
complete model signing message; its requestHash and P256 possession message use
their distinct model domains. `ordinary_refresh_issuance.py` selects the actual
retained enrollment credential, checks current Native policy and raw revocation,
reserves the exact challenge, original DER signature and opaque token, and retains
the real Google response before freezing the `KRPI01` signing input. A restart
returns the same canonical lease without another decode or signature. Conflicting
retries and expired leases are rejected. A lease does not replace the enrollment
credential, financial commitment, epoch or an operation approval.
`ordinary_refresh_service.py` accepts only the closed six-field refresh request.

`play_integrity.py` decodes opaque Standard tokens at Google's fixed HTTPS
endpoint using the Native-authenticated Google policy. It checks the generated
key's requestHash, exact package/version and Play app-signing certificate,
recognized/licensed/device verdicts and trusted freshness. Testing verdicts are
rejected. `google_oauth.py` checks the actual public decoder-policy original and
project/principal/client ID before duplicating the owner-only credential FD.
RS256 signing runs in the same process through the fixed isolated Python runtime's
loaded OpenSSL 3 module; it never pipes the private key to a subprocess. It holds
the actual Root-owned physical `_ssl` original plus the `dladdr`-selected crypto
and TLS dependencies before duplicating the Google credential FD. Owner, mode,
ACL, ancestor, metadata, digest and exact loaded-symbol checks remain mandatory
for private RSA, token-cache reuse and TLS exchange. User-owned Homebrew runtime
files are rejected. Local FD custody does not replace signed runtime admission.
Private
JSON, keys and access tokens are absent from argv, environment and public outputs.

The production `ordinary_worker.py` uses the Native-admitted private descriptor
channel and sealed encoder/runtime/store roles. Its closed phases are `raw`,
`credential` and `refresh`; no request selects an executable, policy or key path.
Linux startup requires the Native parent's held Yama policy to disallow sibling
ptrace before exec; the worker and both encoders independently become nondumpable
before private intake. The shared Python worker also has a macOS process
implementation: it requires one actual native thread, sets and reads back zero
core-file limits, applies `PT_DENY_ATTACH`, then requires a bounded disposable
same-user child to observe real ptrace and task-control/read/inspection denial.
Each task API first passes a self-task control. The probe child closes every
inherited private descriptor before observing its own live parent. A setting
success, generic permission error, missing API or timeout cannot pass. Worker
startup then closes unrelated descriptors, disables inheritance and checks the
fixed pipe/code/signer/Google/store role grammar and held owners before private
intake. No public caller can select a probe PID or alternate OS implementation.

This macOS process check grants no Native issuer authority. The actual installed
owner must still authenticate its signed launch and Python/archive/TLS/native
encoder originals, private custody and current governed policy. Both native encoder entrypoints have matching
macOS protection source implementations; their complete compiled runtime union
and a BOI-owned protected runtime/DATA session remain mandatory before macOS
issuance. This component's disposable
tests do not supply those owners or authorize an issuer launch.

The Core startup adapter validates the same admitted
release, immutable provider originals, signing-key identity and owner-only held
custody before launching the worker. These boundaries have source and component
tests; final integrated release, SDK and financial proof gates remain required.

Run the package tests with the selected Python/OpenSSL runtime:

```sh
PYTHONPATH=src PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -v
```

Scripted Google replies and fixture keys exercise cryptographic and durable-store
behavior. They do not establish real Google decoding, physical qualification or
permission to publish a monetary wallet. Financial publication must verify the
genuine paired State and distinct ordinary Guard under the actual signed release.

Earlier component guidance below records the predecessor 273-byte/KAEA APIs and
diagnostic evidence. Those APIs do not provide a first-release fallback; the old
default WSGI route cannot issue through the current native encoder.

## Predecessor component guidance and retained evidence

# KAGEMUSHA app-attestation verifier boundary

The current first-release design uses an app-owned, nonexportable hardware-backed key to prove that the authentic app approved an operation. It does not install wallet code in the secure element. Android generates a reusable StrongBox P-256 key only after receiving the independently signed enrollment challenge, retains the exact key and chain across lost-result recovery, and later signs an independently specified operation transcript. This verifier authenticates enrollment evidence and binds the actual public key into the governed app certificate. Native operation verification and durable operation replay protection are separate consumers of that certificate. Ordinary StrongBox attestation does not prove a hardware monetary counter, nonforking wallet journal, trusted clock or one-use state. Pixel 6 support must not depend on a custom applet, OMAPI AID, OEM contact, single-use feature flag or `setMaxUsageCount(1)`.

`attestation.py` checks raw Apple App Attest and Android KeyMint evidence separately from the retail issuer. It verifies the Core API's Ed25519-signed 273-byte, 120-second preparation against the same six-field selection used for attestation: client nonce, server nonce, release ID, hardware profile ID, prepared key ID and lane ID. The signature also binds the canonical account and issuer policy. Apple generates its key first and uses the decoded App Attest key ID, SHA256 of the P-256 point. Android KeyMint fixes its challenge at key generation, so its preparation uses an all-zero key-ID sentinel under an independently authenticated Android profile; the verifier derives the actual key reference from the attested point afterward. The platform checks use a trusted clock and exact X.509 root pins: the production provider fixes Apple's attestation path to the published App Attestation Root CA and its separate fraud-receipt path to Apple Root CA - G3, matching [Apple's published attestation object](https://developer.apple.com/documentation/devicecheck/attestation-object-validation-guide). Android accepts published Google roots for its Google profile; an OEM profile instead needs an authenticated exact root and its own positive live revocation checker. Apple evidence must match the pinned App ID and environment, attested key, credential ID and nonce derived from the exact selection. It parses signed validation-category and bundle-version extensions even when the authenticator ED flag is clear, and compares them with independently selected release values when required. If an attestation omits those extensions, the result carries no measured distribution category or bundle version. Android evidence must match the exact package/signing certificate, challenge, locked verified boot, and hardware-generated persistent StrongBox P-256 SIGN/SHA-256 approval key. The supplied certificate list must form one exact authenticated path; the verifier takes KeyDescription from the extension-bearing certificate nearest the pinned root and requires its public key to equal the app-controlled leaf key, so an attacker-added leaf cannot substitute a software key for the hardware claim.

The App Attest nonce extension uses Apple's physical `SEQUENCE { [1] EXPLICIT OCTET STRING }` layout. The verifier requires that tagged form and rejects an untagged nonce. The iPhone 17 Pro Max physical diagnostic passed the independently pinned attestation chain, separate G3 fraud-receipt chain, exact enrollment challenge, and two assertions with counters 1 then 2; that diagnostic used an explicitly unsealed test artifact and does not grant monetary authority.

The first-release app trust claim is the platform-attested app signing identity together with a governance-authorized distribution policy for that identity. The policy digest must come from the authenticated release, not an app-declared version or digest. Provider construction checks that the Apple signing digest equals SHA-256 of the selected App ID, that a selected Apple category/version derives the governed release digest, and that the Android signing digest equals the attested signing-certificate digest. If signed Apple distribution fields are present without being preselected, their digest must still match governance before issuance. These platform attestations do not establish an exact app binary hash, so no exact-binary claim is made here.

`issuance.py` checks an issuer-signed preparation, governed selection, attested key reference, evidence digest and assertion scope before reaching an owner-only SQLite register. New issuance rechecks the same authenticated provider scope and fresh trusted time under the write lock immediately before signing, so a request queued past the preparation deadline cannot issue. The register keeps one original certificate and audit evidence per preparation, supports identical lost-result recovery after preparation expiry, rejects conflicting retries, and reserves each attested device key for one enrollment. `iroha_data_model`'s `kagemusha_app_certificate_encoder_v1` is an offline encoder source that signs `KagemushaAppEnrollmentCertificateV1` using Iroha's own Rust model and `norito::encode_canonical`; it accepts a versioned fixed-field local request and an inherited signing-key descriptor. It cannot replace release authentication or raw platform verification.

The signed certificate contains the actual SHA-256 key ID derived from the attested SEC1 point alongside the device-key reference. Apple's signed preparation uses that same key ID. Android's signed preparation uses a zero key-ID sentinel because KeyMint fixes the challenge at key generation; its certificate still signs the actual key ID after raw evidence verification.

`service.py` defines one request/recovery contract: `POST /v1/kagemusha/app-certificates` with `Content-Type: application/json`. The exact body fields are `operation` (`issue` or `recover`), `account_canonical`, `signed_preparation_base64`, `selection`, `platform`, and `platform_evidence_base64`. `selection` has six lowercase 64-digit hex fields: `client_nonce_hex`, `server_nonce_hex`, `release_id_hex`, `hardware_profile_id_hex`, `attested_key_id_hex`, and `lane_id_hex`. Android uses 32 zero bytes for `attested_key_id_hex`. For Apple, `platform_evidence_base64` is the raw App Attest CBOR object. For Android, it is exactly `KMCA` + byte `01` + certificate count `u8` + repeated `DER length u32 big-endian | DER bytes`, leaf first, 2–8 certificates, at most 128 KiB. A success returns `certificate_base64` and `certificate_sha256_hex`; exact recovery invokes no signer. Unknown or duplicate fields and noncanonical byte encodings are rejected. The WSGI boundary requires a deployment-owned authenticated caller callback, such as a trusted Core API mTLS identity. Mobile clients should reach this internal service through the authenticated Core API facade; this verifier never treats a client-supplied HTTP header as authentication. The default service has no caller authorizer, evidence provider, store or signer and returns `503 issuer_unavailable` for valid requests.

The component still has **no production certificate-issuing route**: its WSGI handler can only issue when a deployment supplies a trusted `EvidenceProvider` and signer. A raw `RawPlatformProof` is not an issuance token. `provider.py` composes independently selected policy with the raw Apple attestation and signed CMS receipt, or Android KeyMint with the selected root's live revocation authority. Its caller must authenticate that policy from release governance; the provider does not load deployment configuration or connect a signer. Apple assertion verification and durable server-side counter advance are separate later-use checks; the enrollment request does not contain an assertion. The signer must remain disconnected from production keys until the authenticated release/configuration and signer deployment are implemented and tested. The later issuer `/start` check must bind the signed certificate's point/reference to the canonical Norito qualification, profile, lane and governed app-policy digest. Add real Apple and Android device fixtures and an external review of chain and ASN.1 handling. An app-supplied digest or JSON field cannot fill these gaps.

`revocation.py` fetches Google's exact HTTPS attestation-status resource without redirects or a proxy, parses its bounded documented schema, and rejects a listed serial at any depth of an already verified Google-root chain. It fails closed when the live status is unavailable. `provider.py` invokes it after raw chain verification and accepts only Google's published root pins for that profile. An `OemKeyMintPolicy` accepts an operator-authenticated non-Google root only with a deployment-owned verifier that returns `True` after a fresh authoritative check of the complete chain; absent, failed or negative checks block issuance and recovery. This policy hook does not supply a monotonic offline monetary journal or one-use hardware state. Google's profile includes the exact 2016 factory root still listed in [Android's attestation guidance](https://developer.android.com/privacy-and-security/security-key-attestation): OpenSSL verifies its chain and order with time checks disabled, then the verifier checks every non-root certificate at the trusted current time and the pinned expired root before its historical expiry. Current revocation remains mandatory. Pixel 6 approval-key admission prefers ordinary StrongBox attestation and permits genuine TEE only when the authenticated policy accepts it and does not require rollback-resistance or limited-use tags.

`verify_apple_assertion` checks exact server-selected client data, the stored attested key and App ID, bounded CBOR/DER, Apple's ES256 signature over `SHA256(authenticatorData || SHA256(clientData))`, and a counter strictly greater than its previous value. ECDSA-SHA256 hashes that nonce as its message; a signature over the unhashed concatenation is rejected. Apple assertion extensions use `validationCategory` and `bundleVersion`, while attestation extensions use `apple_validation_category_01` and `apple_bundle_version_01`; the parser rejects either pair in the wrong object. `DurableAppleAssertionCounterStore` stores the counter with a full-sync SQLite transaction and consumes each client-data digest once across all registered keys and process restarts. Apple permits counter gaps; neither this server register nor an assertion by itself proves that an offline monetary head cannot be rolled back. These checks are not yet connected to issuance and have only synthetic fixtures.

For the physical iPhone XCTest, export its two public JSON attachments and run `python3 -m iroha_app_attestation.verify_physical_apple --enrollment-json <path> --assertions-json <path> --root-der <Apple App Attestation Root CA DER> --receipt-root-der tests/fixtures/apple_root_ca_g3.der --app-id <TeamID.BundleID> --environment development --expected-bundle-version <version-from-the-built-app>`. Convert the [App Attestation root published by Apple](https://www.apple.com/certificateauthority/Apple_App_Attestation_Root_CA.pem) from PEM to DER with `openssl x509 -in <pem> -outform der -out <der>`. The checked-in [Apple Root CA - G3](https://www.apple.com/certificateauthority/AppleRootCA-G3.cer) certificate pins the separate CMS fraud-receipt chain; the checker also binds that receipt to the exact attested credential certificate. It compares the attachment version with the independently selected build version, recomputes the category-3 development-signed-app release digest, binds both signed selections to the fixed XCTest nonce/release/profile/lane and structural S fields, and requires exact 0→1→2 counters. The receipt's creation time must be within five minutes of verification, so run the checker promptly after XCTest exports its attachments. Because the observed iPhone assertions have no signed version extension, that bundle-version check is an artifact/vector check, not an Apple measurement of the installed binary. This is device evidence, not certificate issuance or offline-money admission.

The verifier's caller callback must return the exact deployment-pinned Core mTLS identity. False, empty, and other identities are rejected. Omitting the expected identity closes the route even if a callback is supplied.

The raw checks follow [Apple's App Attest validation steps](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server), [Android's KeyMint attestation schema](https://source.android.com/docs/security/features/keystore/attestation), and [Google's attestation revocation policy](https://developer.android.com/privacy-and-security/security-key-attestation). Apple documents category values 1, 2, 3, 4, 5, 6 and 10; this ordinary-iPhone admission profile intentionally permits only 2 (TestFlight), 3 (development), 4 (App Store) and 5 (enterprise/ad-hoc) when a governed signed category is available. Categories 1, 6 and 10 are not admitted for an ordinary retail iPhone app. Android optional authorization tags documented in KeyMint 100–500 are parsed with DER type and ordering checks. The Android approval profile requires matching attestation and KeyMint security levels selected explicitly by the authenticated TEE/StrongBox allowlist, hardware-generated EC/P-256 SIGN/SHA-256 authorizations, exact app identity and locked verified boot. It rejects usageCountLimit keys because the approval key must persist across operations; rollback-resistance is optional and grants no wallet-state authority. Run local parser, cryptographic-boundary and durable-store tests with `PYTHONPATH=src python3 -m unittest discover -s tests -v`. The Rust encoder must be built and tested against the matching Iroha source revision after the current shared-source build hold.
