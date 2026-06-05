# ISO XMLDSig Trust Bundle Templates

These files are templates for `scripts/iso_trust_bundle_verify.py`. They are not
production trust anchors. The DER values are synthetic DER SEQUENCE envelopes so
the verifier can exercise digest, base64, duplicate, and emitted-profile wiring
without embedding a real rail CA package.

Validate all templates without contacting a network endpoint:

```bash
python3 scripts/iso_trust_bundle_verify.py \
  --bundle fixtures/iso20022/trust_bundles/swift_cbpr_plus.preprod.example.json \
  --allow-synthetic-der \
  --summary-out /tmp/swift-cbpr-plus-trust-summary.json
```

Before production use, replace the example source metadata, trust-anchor DER or
pins, CRL material, OCSP material, and policy OIDs with the current rail
package and omit `--allow-synthetic-der`. Only real-material bundles can emit
profile overrides with `--emit-profile-json`; synthetic-template validation is
summary-only. Production source URLs must be clean HTTPS provenance URLs:
credentials, params, query strings, fragments, malformed bracket syntax,
control characters, localhost, and local/private IP literals are rejected
unless the explicit local-audit `--allow-insecure-source-url` override is used.
`source.retrieved_at` must be a timezone-aware ISO 8601 timestamp and cannot be
in the future. DER material labels must be unique within each material class.
The verifier performs lightweight semantic DER-shape checks for X.509
certificates, X.509 CRLs, and OCSPResponse wrappers; Torii remains the
authoritative cryptographic and policy verifier at startup and message
admission.
