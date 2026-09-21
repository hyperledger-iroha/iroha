# Final-promotion cosign cryptographic test fixture

These public upstream fixtures exercise the actual pinned cosign executable with
a certificate, signature, Rekor v2 inclusion proof and signed RFC 3161 timestamp.
They are not SoraFS
approval evidence. The artifact has no final-promotion statement domain, and its
certificate identity is never acceptable as a deployment's implicit identity.
The final checker separately owns the canonical promotion subject projection.

`sources.json` records exact upstream commit URLs, lengths and SHA-256 values.
The artifact, v0.3 bundle and explicit staging trust fixture originate in the
Apache-2.0-licensed
[Sigstore conformance suite](https://github.com/sigstore/sigstore-conformance/tree/bf6b322ef65839216ec8853287032750e1f4b92d/test/assets/bundle-verify/rekor2-happy-path).
Copyright belongs to the respective Sigstore authors; these files contain only
public test material. The fixture's root must never become a production default.

Run the crypto tests with both independent operator inputs:

```sh
python -m pytest scripts/tests/sorafs_final_promotion_cosign_crypto_test.py \
  --sorafs-cosign-verifier /absolute/reviewed/cosign \
  --sorafs-cosign-verifier-sha256 REVIEWED_EXECUTABLE_SHA256
```

The test suite does not discover or trust an executable from `PATH`, download
material, update a trust cache, or initiate signing. It skips the executable
tests when both options are absent and rejects a partial pair. Release
qualification must supply both options and require zero skips. The local fixture
integrity test always runs. All input mutations occur in private temporary copies.
