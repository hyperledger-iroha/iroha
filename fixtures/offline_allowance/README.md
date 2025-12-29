# Offline allowance fixtures

This directory contains the canonical OA10 fixtures generated from
`scripts/offline_topup/spec.example.json`. Each allowance in the spec renders a
subdirectory with the Norito certificate/instruction plus a JSON summary,
while `allowance_fixtures.manifest.json` records the certificate ids and file
paths that CI/tests load.

Regenerate the bundle any time the example spec changes:

```
scripts/offline_topup/run.sh \
    --spec scripts/offline_topup/spec.example.json \
    --output fixtures/offline_allowance
```

The spec pins operator/spend keys, blindings, attestation reports, and HMS
nonce metadata so rerunning the command is deterministic across machines.
