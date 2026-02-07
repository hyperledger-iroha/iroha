---
lang: my
direction: ltr
source: docs/source/sorafs_chunk_range_smoketest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf6279ce239a097aaf93c11264e422ebde0e579087a0b8c0760209ff2195bf3c
source_last_modified: "2026-01-22T14:35:37.504781+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Chunk-Range Smoketest Plan (Draft)
summary: Quick validation workflow for chunk-range endpoints and orchestrator integration.
---

# SoraFS Chunk-Range Smoketest Plan (Draft)

## Purpose

- Provide fast validation for gateway chunk-range support post-deployment.
- Ensure orchestrator + stream tokens + proofs behave correctly in a small scenario.

## Workflow Outline

1. Generate a stream token via the built-in CLI (`iroha app sorafs storage token issue`) and export the manifest chunk plan by running `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --json-out manifest_report.json` (the report includes `chunk_fetch_specs`).
2. Run `iroha app sorafs fetch --manifest manifest.to --plan manifest_report.json --manifest-id <manifest_digest_hex> --gateway-provider "name=primary,provider-id=<hex32>,base-url=https://gateway.example,stream-token=<base64>" --max-peers 4 --retry-budget 3`.
3. CLI verifies chunks + proofs, records summary metrics and (optionally) writes the assembled payload (`--output`) / JSON report (`--json-out`).
4. Compare results with expected digest/metrics thresholds.

### Issuing stream tokens

Use the manifest report from `iroha app sorafs toolkit pack` and the provider id from the
gateway registry/admission record to mint a scoped token for the gateway under test.
The CLI wraps Torii’s `/v1/sorafs/storage/token` endpoint and takes care of the
`X-SoraFS-Client`/`X-SoraFS-Nonce` headers for you:

```bash
MANIFEST_REPORT_JSON="manifest_report.json"
MANIFEST_ID_HEX="$(jq -r '.manifest_digest_hex' "${MANIFEST_REPORT_JSON}")"
PROVIDER_ID_HEX="<provider-id-hex>"

iroha app sorafs storage token issue \
  --manifest-id "${MANIFEST_ID_HEX}" \
  --provider-id "${PROVIDER_ID_HEX}" \
  --client-id smoke-ci \
  --ttl-secs 900 \
  --max-streams 4 \
  --rate-limit-bytes 134217728 \
  --requests-per-minute 12
```

When `--nonce` is omitted the command prints the generated nonce (for audit logs) and emits a
JSON document describing the token body, signature, and encoded payload:

```jsonc
{
  "token": {
    "body": {
      "token_id": "4f9f1c7d2e5e4a86c4d5ae9919cf0da2",
      "manifest_cid_hex": "b41600aac9ff0d3d0fc9f48e3a5659c7c7a9",
      "provider_id_hex": "00112233445566778899aabbccddeeff",
      "profile_handle": "chunk_profile:ldx4",
      "max_streams": 4,
      "ttl_epoch": 1739210400,
      "rate_limit_bytes": 134217728,
      "issued_at": 1739209500,
      "requests_per_minute": 12,
      "token_pk_version": 1
    },
    "signature_hex": "29f71a5d3f4b56c77d44cba2c5ff5c8181ae",
    "encoded": "AABbZGF0YURyb3BTYW1wbGVQYXlsb2FkCg=="
  },
  "token_base64": "AABbZGF0YURyb3BTYW1wbGVQYXlsb2FkCg=="
}
nonce: 4c1f8e9d97c84d61b9d3c8c5
```

Use either `token.encoded` or `token_base64` as the `stream-token=<base64>` component in
`--gateway-provider` when invoking `iroha app sorafs fetch`. The provider hex identifier in
that flag must match the `provider_id_hex` you passed to the token issuer or the gateway
will reject the request.

## Expected Output

```json
{
  "manifest_id": "d1d6c1b12ba2cd505c6c738bcb12de9dcda6658fcfaa739bf482ba4bf664f7f3",
  "manifest_cid": "6c4347...c7a9",
  "chunk_count": 128,
  "fetched_bytes": 33554432,
  "provider_count": 1,
  "providers": ["primary"],
  "provider_reports": [
    { "provider_id": "primary", "alias": "primary", "successes": 128, "failures": 0, "disabled": false }
  ],
  "chunk_receipts": [
    { "chunk_index": 0, "provider_id": "primary", "alias": "primary", "attempts": 1 }
    // ...
  ]
}
```

## Metrics Thresholds

- Success chunk percentage >= 99%
- Average latency < 50 ms (hot-cache expectation; adjust per environment)
- Proof failures = 0

## CI Integration

- Add a Buildkite step `ci/sorafs-chunk-range-smoketest` that executes the CLI against the
  staging gateway immediately after deployment. Reference the shared template described in
  `docs/source/sorafs_ci_templates.md` and pass the manifest/token fixtures via pipeline
  secrets (`SORA_FS_GATEWAY_URL`, `SORA_FS_STREAM_TOKEN`).
- Archive the CLI JSON output and attach it to the build summary so operators can review
  latency/throughput without rerunning the test.
- Wire the step as a merge gate for PRs touching `docs/source/sorafs_gateway_*`,
  `crates/sorafs_*`, or `ci/sorafs_*` scripts.

## Cold-Cache Thresholds

- For cold-cache validation (first-run or cache flush scenarios) relax the latency guard to
  `avg_latency_ms < 150` and `p95_latency_ms < 250`. Proof failure tolerance remains zero.
- Extend the CLI to accept `--profile cold` so the report includes the profile label and the
  CI job can choose the appropriate threshold set.
- Record the cache state (`warm`/`cold`) in the JSON artifact (`"cache_state": "cold"`).

## Multi-Gateway Scenario

- Once the orchestrator MVP lands, expand the smoketest to iterate over the provider list
  advertised in the manifest (`manifest.providers[]`). Use the orchestrator’s weighted
  scheduler to fetch from each gateway and report per-gateway latency/failure metrics.
- Aggregate results into a consolidated JSON report with per-gateway entries and a merged
  success summary. Update the CI workflow to fail if any gateway falls below the success
  threshold while still highlighting individual latency regressions.
