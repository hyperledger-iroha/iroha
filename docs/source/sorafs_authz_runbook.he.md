---
lang: he
direction: rtl
source: docs/source/sorafs_authz_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c536bc1296789d05adce8d6a2f4928c362fad854ed894ca76a6064a296ef2f4
source_last_modified: "2026-01-25T13:30:16.933810+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS/SoraNet Authz Runbook

This note summarises the authorization and abuse controls around SoraFS control-plane actions and the SoraNet privacy ingest endpoints so operators can provision tokens, bind providers, and rotate credentials without guesswork.

## Surfaces and tokens

- SoraFS instructions are gated by dedicated tokens: pin register/approve/retire/alias, capacity declare/telemetry/dispute, replication order issue/complete, pricing set, and provider credit upsert.
- Provider→account bindings must be present before issuing replication orders or submitting capacity telemetry; use the governance config seed or the `RegisterProviderOwner`/`UnregisterProviderOwner` instructions to manage bindings.
- Repair worker endpoints (`/v1/sorafs/audit/repair/{claim,heartbeat,complete,fail}`) require signed `RepairWorkerSignaturePayloadV1` requests from a worker account (i105 account id/signatory key) that holds `CanOperateSorafsRepair { provider_id }`. The signed payload includes `manifest_digest` and must match `manifest_digest_hex` in the request; provider owners are auto-granted this permission and may delegate it via `GrantPermission`; revoke with `RevokePermission` during rotation.
- The SoraFS storage pin API (`/v1/sorafs/storage/pin`) enforces bearer tokens, CIDR allow-lists, and a token-bucket limit from `sorafs.storage.pin`.
- SoraNet privacy ingest endpoints (`/v1/soranet/privacy/{event,share}`) require `X-SoraNet-Privacy-Token` (or `X-API-Token`), a non-empty CIDR allow-list, and the token/burst limits under `torii.soranet_privacy_ingest`; requests outside the namespace or over budget are rejected before metrics ingestion.

## Telemetry submitters and provider overrides

- `governance.sorafs_telemetry.require_submitter` and `require_nonce` default to `true`, forcing telemetry windows to come from authorised accounts with replay protection. When `require_nonce=false`, windows without a nonce are accepted but any provided nonces are still checked for replay.
- `submitters` defines the global allow-list; `per_provider_submitters` overrides it for specific providers when the submitting account differs from the default.
- Capacity telemetry still enforces provider ownership, window hygiene, and nonce replay detection; spoofed or mismatched owners are rejected with labelled telemetry.

## Torii ingress guards

- `sorafs.storage.pin`: require token, allowed CIDRs, and the optional ban-aware rate limit. Tokens are carried in `Authorization: Bearer …` or `X-SoraFS-Pin-Token`.
- `torii.soranet_privacy_ingest`: disabled by default; enabling requires a token list and CIDR scope (empty list denies). The rate limiter uses `rate_per_sec`/`burst`, keyed by token/IP, and emits `soranet_privacy_ingest_reject_total{endpoint,reason}` on rejects.
- Sample configuration:

```toml
[sorafs.storage.pin]
require_token = true
tokens = ["pin-prod-token"]
allow_cidrs = ["10.10.0.0/16"]
[sorafs.storage.pin.rate_limit]
max_requests = 30
window = "60s"
ban = "5m"

[torii.soranet_privacy_ingest]
enabled = true
require_token = true
tokens = ["privacy-prod-token"]
allow_cidrs = ["10.20.0.0/16", "fd00:20::/48"]
rate_per_sec = 5
burst = 10

[governance.sorafs_telemetry]
require_submitter = true
require_nonce = true
submitters = ["<katakana-i105-account-id>"]
per_provider_submitters = { "deadbeef..." = ["<katakana-i105-account-id>"] }
```

## CLI/REST quick reference

- Register a pin manifest with the CLI, carrying the alias proof when required:
  ```bash
  iroha_cli app sorafs pin register \
    --manifest /var/lib/sorafs/manifests/pin.to \
    --chunk-digest 0123abcd... \
    --submitted-epoch 0 \
    --config /etc/iroha/config.toml
  ```
- Audit or rotate tokens/allow-lists by reloading the Torii config and verifying the rejects:
  ```bash
  curl -H "X-SoraNet-Privacy-Token: privacy-prod-token" \
    https://torii.example.com/v1/soranet/privacy/event \
    --data-binary @tests/fixtures/privacy_event.json
  ```
- Confirm telemetry submitter bindings before sending windows (rejects surface as `unauthorised_submitter[_provider]` in logs/telemetry):
  ```bash
  iroha_cli ledger query --config /etc/iroha/config.toml \
    --name governance.sorafs_telemetry.submitters
  ```

## Operator checklist

1. Bind provider owners in genesis or via `RegisterProviderOwner`; confirm with the provider-owner query before accepting telemetry.
2. Set `governance.sorafs_telemetry.submitters` and any `per_provider_submitters` overrides; keep `require_nonce=true` unless running a controlled replay drill.
3. Delegate `CanOperateSorafsRepair` to repair worker accounts before enabling automation, and rotate by revoking the permission plus reissuing worker keys (no admin-only bypass for repair actions).
4. Provision pin tokens and CIDR scopes under `sorafs.storage.pin`; rehearse the rate-limit/ban response with a staging token before rotating production values.
5. Enable `torii.soranet_privacy_ingest` only after populating `tokens` and `allow_cidrs`; rotate credentials by reloading the config and watch `soranet_privacy_ingest_reject_total` for namespace/token rejects.
6. Verify ingress with a signed sample request (e.g., `curl -H "X-SoraNet-Privacy-Token: privacy-prod-token" …/v1/soranet/privacy/event`) and confirm the endpoint returns `202 Accepted`.
7. Monitor `soranet_privacy_ingest_reject_total{reason}`, `soranet_privacy_throttles_total`, and the SoraFS quota metrics to catch abuse early; keep the checklist alongside change tickets for token/allow-list rotations.
