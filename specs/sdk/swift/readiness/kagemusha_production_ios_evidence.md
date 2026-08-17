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
and raw App Attest attestation and assertion objects. The policy supplies the
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
  App ID/RP hash, extension policy, and positive assertion counter; and
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

Current Apple revocation status and replay state are online facts, not facts a
static root bundle can prove. The validator therefore also requires a bounded
canonical
`iroha.kagemusha.ios.app_attest_online_freshness_consumption_receipt.v1`
signed by an Ed25519 authority key distinct from the lab signer. The receipt
binds the exact signed-evidence digest, policy digest, catalog-selected release
manifest digest, platform-evidence digest, both canonical client challenges,
both App Attest objects, both embedded server nonces, the attestation and
assertion message nonces, key ID, ordered certificate digests, previous and
new assertion counters, issuance/consumption/expiry times, and a distinct
one-time consumption identifier. The new counter must equal authenticatorData
and strictly exceed the authority-attested prior counter. Receipt lifetime and
the authority's Apple-revocation check are each limited to five minutes.

This receipt is a statelessly verifiable claim. Its signature does not prove
that the external service actually maintains durable one-time issuance,
consumption, per-key counter, or current Apple CRL/OCSP state. Production
operations must therefore review and provision that service and its key
lifecycle before installing its trusted key. Once provisioned, a valid signed
receipt is the validator's success path; an absent, malformed, untrusted,
stale, or substituted receipt fails closed. A lab signature,
certificate-byte presence, self-reported time, receipt Boolean, or
`app_attest_used:true` never satisfies the external requirement.

The physical capture application must use `DCAppAttestService.generateKey`,
then `attestKey(_:clientDataHash:)` over the exact canonical attestation client
data, and finally `generateAssertion(_:clientDataHash:)` over the exact
canonical assertion client data. The existing
`KagemushaRequestAuthorizationPreparation.authorizeWithIosAppAttest` Swift API
already exercises the assertion API for transaction authorization, but the
candidate evidence lab does not yet perform the production key-attestation
capture. No synthetic CBOR or software P-256 key is acceptable outside tests.

Promotion additionally requires the production policy through
`KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY`. The gate snapshots that
policy, the trusted lab-signing key, and both reviewed iOS validators before
validation. The standalone production checker additionally requires
`--freshness-receipt`, `--trusted-freshness-key-id`, and
`--trusted-freshness-public-key`; the promotion gate must snapshot equivalent
root-custodied inputs before activation. It also requires the production envelope's
`release_manifest_sha256` to equal the enclosing catalog directory name.

Mutation coverage lives in
`scripts/tests/check_kagemusha_candidate_ios_evidence_test.py` and includes
policy ID/hash substitution, malformed policy collection types, CBOR container
count overflow, control bytes, omitted code-sign binding, raw-tree loss,
assertion-signature tampering, RP-ID substitution, fake and expired
certificates, static revocation, nonce substitution, receipt-signature and
evidence-digest substitution, stale online revocation status, counter rollback,
and one-time consumption-ID reuse. The repository's pinned Apple App
Attestation P-384 root is a real parser/signature fixture; synthetic certificate
chains remain test-only and are never production evidence.
The repository does not contain a captured Apple leaf/intermediate/authData
qualification fixture or a provisioned online authority; production activation
therefore still requires a physical App Attest capture plus operational review
of the live authority's durable replay/counter and Apple-status state.
