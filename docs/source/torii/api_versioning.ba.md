---
lang: ba
direction: ltr
source: docs/source/torii/api_versioning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e27b4aa44b025737d3e8dfed751fc5a0bc1dd12807f77a887f46ce830025c8
source_last_modified: "2026-01-05T09:28:12.103512+00:00"
translation_last_reviewed: 2026-02-07
---

# Torii API Versioning

Torii negotiates a semantic API version (`major.minor`) on every request using
the `x-iroha-api-version` header. If the header is absent, Torii assumes the
latest supported version and echoes it back alongside the full support matrix.

- Supported today: `1.0`, `1.1` (default).
- Proof/staking/fee surfaces require at least `1.1`; older versions receive
  `426 UPGRADE_REQUIRED` with `code="torii_api_version_too_old"`.
  `1893456000`, 2030-01-01 UTC) and continue to serve responses while logging a
  warning.
- Response headers always include `x-iroha-api-version`,
  `x-iroha-api-supported`, and `x-iroha-api-min-proof-version` so clients can
  adapt without hard-coding values.
- Error codes are stable:
  - `torii_api_version_invalid` — header is not a semantic version.
  - `torii_api_version_unsupported` — version not in the server support set.
  - `torii_api_version_too_old` — version below the per-surface minimum.

The `/v1/api/versions` endpoint surfaces the support matrix (`default`,
`supported`, `min_proof_version`, optional `sunset_unix`) for diagnostics and
SDK defaults. Clients should send the header explicitly; the CLI now defaults
to `1.1` and threads overrides via `torii_api_version` in `client.toml`.

The `/api_version` endpoint returns the active block header version string as
plain text. If genesis is not yet committed, it responds with
`503 SERVICE_UNAVAILABLE` and the body `genesis not applied`.
