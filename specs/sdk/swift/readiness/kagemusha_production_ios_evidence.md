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
  normalization used by the native bridge.

That is a safe verification substrate, not production qualification. The
validator deliberately returns a blocker until it also directly verifies the
Apple X.509 chain to the policy roots, certificate validity and revocations,
the leaf `1.2.840.113635.100.8.2` nonce extension against the attestation
challenge, and independently issued freshness/replay state. A lab signature,
the presence of certificate bytes, or Boolean claims such as
`app_attest_used:true` never satisfy those requirements.

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
validation. It also requires the production envelope's
`release_manifest_sha256` to equal the enclosing catalog directory name.

Mutation coverage lives in
`scripts/tests/check_kagemusha_candidate_ios_evidence_test.py` and includes
policy ID/hash substitution, control bytes, omitted code-sign binding, raw-tree
loss, assertion-signature tampering, RP-ID substitution, and counter rollback.
