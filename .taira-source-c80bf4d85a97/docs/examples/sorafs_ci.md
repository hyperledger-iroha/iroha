---
title: SoraFS CI Cookbook
summary: Reference build job plus protected aggregate release-manifest authentication.
---

# SoraFS CI Cookbook

Keep content preparation separate from release authentication. An ordinary build
job can create and verify deterministic SoraFS artifacts:

```yaml
name: sorafs-candidate

on:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  candidate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<reviewed-full-sha>
      - name: Package and verify payload
        run: |
          mkdir -p artifacts
          sorafs_cli car pack \
            --input=payload.bin \
            --car-out=artifacts/payload.car \
            --plan-out=artifacts/chunk_plan.json \
            --summary-out=artifacts/car_summary.json
          sorafs_cli manifest build \
            --summary=artifacts/car_summary.json \
            --manifest-out=artifacts/manifest.to
          sorafs_cli proof verify \
            --manifest=artifacts/manifest.to \
            --car=artifacts/payload.car \
            --chunk-plan=artifacts/chunk_plan.json \
            --summary-out=artifacts/proof.json
      - uses: actions/upload-artifact@<reviewed-full-sha>
        with:
          name: sorafs-unsigned-candidate
          path: artifacts/
```

After the release pipeline reproduces the candidate, generates SBOM and scan
evidence, and writes canonical `release_manifest.json`, a protected runner signs
that aggregate manifest:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/release/release_manifest.json \
  --external-signer /run/sorafs-release/ed25519-sign \
  --signing-public-key /run/sorafs-release/release.ed25519.pub \
  --trusted-signing-fingerprint "$REVIEWED_SIGNER_SHA256" \
  --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
  --trusted-release-manifest-verifier-sha256 "$REVIEWED_VERIFIER_SHA256"
```

The signer adapter should use the governed PKCS#11/HSM key and write exactly 64
raw signature bytes. Archive the aggregate manifest, raw signature, raw public
key, reviewed signer fingerprint, pinned validator SHA256, and native
verification receipt. OIDC/cosign may attest provenance in a separate job; that
bundle does not replace release authenticity.

Deterministic content fixtures live in
`fixtures/sorafs_manifest/ci_sample`. Compare CAR, chunk-plan, manifest, and proof
outputs byte-for-byte. Never substitute fixture credentials, keys, signatures,
fingerprints, or verifier digests for production inputs.
