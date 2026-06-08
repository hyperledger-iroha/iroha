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
package, keep an explicit `embedded_signature_policy`, record every
list-typed material field as an array even when it is intentionally empty, and
omit `--allow-synthetic-der`. Only real-material bundles can emit profile
overrides with `--emit-profile-json`; synthetic-template validation is
summary-only, and profile emission also rejects local-audit source URL
overrides, record-only policy overrides, and placeholder source metadata such as `dummy`,
`fake`, `placeholder`, `replace-before-production`, `sample`, `template`, or
reserved hosts like `.example`, `example.com`, `example.net`, `example.org`,
and `example.invalid`.
Profile emission requires an explicit `--max-source-age-days` freshness budget
and rejects stale `source.retrieved_at` values under that budget. The summary
records that budget so evidence and readiness gates can reject omitted,
malformed, or weaker source-freshness policy later, and can recompute whether
`profile_json_emittable` still matches the archived source evidence and the
trust-verifier diagnostic flags.
Production source URLs must be clean HTTPS provenance URLs:
credentials, params, query strings, fragments, malformed bracket syntax,
control characters, localhost, local/private IP literals, known local/private
rebinding hostnames, legacy IPv4 numeric notation, and IPv6 transition
addresses embedding non-global IPv4 addresses are rejected unless the explicit
local-audit `--allow-insecure-source-url` override is used.
The top-level `source` object must be recorded; omission and explicit `null`
are both malformed. `source.authority` and `source.version` are required, and
`source.retrieved_at` must be a timezone-aware ISO 8601 timestamp and cannot be
in the future. DER material labels must be unique within each material class.
Every DER material object must declare a `sha256` value, and the verifier
checks it against the canonical decoded `der_base64` bytes.
The verifier performs lightweight semantic DER-shape checks for X.509
certificates, X.509 CRLs, and OCSPResponse wrappers; Torii remains the
authoritative cryptographic and policy verifier at startup and message
admission.
