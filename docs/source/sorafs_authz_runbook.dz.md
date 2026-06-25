---
lang: dz
direction: ltr
source: docs/source/sorafs_authz_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 043613f51b25778b8d2ded887b05316c14811c93cca4d536af5a0d988f10d5dc
source_last_modified: "2026-06-24T19:12:34.494861+00:00"
translation_last_reviewed: 2026-06-24
---

# SoraFS/SoraNet Authz Runbook

This note summarises the authorization and abuse controls around SoraFS control-plane actions and the SoraNet privacy ingest endpoints so operators can provision tokens, bind providers, and rotate credentials without guesswork.

## Surfaces and tokens

- `RegisterPinManifest` is public on the universal lane. Submission collects the SoraFS public pin fee from the submitter into the governance treasury, records the fee metadata on the pin record, activates the manifest immediately, and auto-issues the minimum replication order whenever active capacity declarations can satisfy the manifest policy.
- The remaining SoraFS instructions are gated by dedicated tokens: pin approve/retire/alias, capacity declare/telemetry/dispute, replication order issue/complete, pricing set, and provider credit upsert. `ApprovePinManifest` remains available to attach or ratify the council envelope on an already active manifest.
- Provider→account bindings must be present before issuing replication orders or submitting capacity telemetry; use the governance config seed or the `RegisterProviderOwner`/`UnregisterProviderOwner` instructions to manage bindings.
- Repair worker endpoints (`/v1/sorafs/audit/repair/{claim,heartbeat,complete,fail}`) require signed `RepairWorkerSignaturePayloadV1` requests from a worker account (i105 account id/signatory key) that holds `CanOperateSorafsRepair { provider_id }`. The signed payload includes `manifest_digest` and must match `manifest_digest_hex` in the request; provider owners are auto-granted this permission and may delegate it via `GrantPermission`; revoke with `RevokePermission` during rotation.
- The SoraFS storage pin API (`/v1/sorafs/storage/pin`) requires a matching approved paid pin registry record for the manifest digest, chunk profile, content length, policy, chunk plan digest, and fee payer. The recorded fee asset, treasury, and amount are treated as the committed on-chain receipt, so later governance pricing or treasury changes do not invalidate an already-paid manifest. It no longer treats bearer tokens or CIDR allow-lists as the source of admission authority; quota limits still apply before ingest.
- Local moderation quarantine review, release, and encrypted object store/read endpoints require canonical request signatures from accounts holding the `sorafs_moderation_operator` role. Keep this as a dedicated empty role for the Torii API gate; do not attach broad ledger permissions to it unless a separate governance change requires them.
- SoraNet privacy ingest endpoints (`/v1/soranet/privacy/{event,share}`) require `X-SoraNet-Privacy-Token` (or `X-API-Token`), a non-empty CIDR allow-list, and the token/burst limits under `torii.soranet_privacy_ingest`; requests outside the namespace or over budget are rejected before metrics ingestion.

## Telemetry submitters and provider overrides

- `governance.sorafs_telemetry.require_submitter` and `require_nonce` default to `true`, forcing telemetry windows to come from authorised accounts with replay protection. When `require_nonce=false`, windows without a nonce are accepted but any provided nonces are still checked for replay.
- `submitters` defines the global allow-list; `per_provider_submitters` overrides it for specific providers when the submitting account differs from the default.
- Capacity telemetry still enforces provider ownership, window hygiene, and nonce replay detection; spoofed or mismatched owners are rejected with labelled telemetry.

## Torii ingress guards

- `sorafs.storage.pin`: legacy token and CIDR fields remain parseable for old configs, but storage pin admission is paid-registry based. Keep operational rate limits enabled so a valid paid pin cannot be replayed into an unbounded local ingest burst.
- `torii.soranet_privacy_ingest`: disabled by default; enabling requires a token list and CIDR scope (empty list denies). The rate limiter uses `rate_per_sec`/`burst`, keyed by token/IP, and emits `soranet_privacy_ingest_reject_total{endpoint,reason}` on rejects.
- Sample configuration:

```toml
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
submitters = ["<i105-account-id>"]
per_provider_submitters = { "deadbeef..." = ["<i105-account-id>"] }
```

## CLI/REST quick reference

- Register a pin manifest with the CLI. The manifest submitter must hold the configured SoraFS pin-fee asset as well as the normal transaction fee asset, and must still carry the alias proof when required:
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
- Register the moderation operator role once per network. Registration grants
  the role to the registrant, so use a governance/admin account that should be
  allowed to bootstrap the operator roster:
  ```bash
  iroha_cli ledger role register \
    --id sorafs_moderation_operator \
    --config /etc/iroha/config.toml
  ```
- Grant the role to each canonical moderation operator account before enabling
  quarantine review/release or encrypted payload readback. The role gate uses
  canonical `AccountId` membership, so bind any human-readable aliases
  separately from this step:
  ```bash
  iroha_cli ledger account role grant \
    --id <i105-account-id> \
    --role sorafs_moderation_operator \
    --config /etc/iroha/config.toml
  ```
- Verify the roster before opening operator traffic:
  ```bash
  iroha_cli ledger account role list \
    --id <i105-account-id> \
    --config /etc/iroha/config.toml
  ```
- Rotate or suspend an operator by revoking the role, then confirm signed
  quarantine review/release and object readback calls return `403 Forbidden`
  for the retired account:
  ```bash
  iroha_cli ledger account role revoke \
    --id <i105-account-id> \
    --role sorafs_moderation_operator \
    --config /etc/iroha/config.toml
  ```

## Operator checklist

1. Bind provider owners in genesis or via `RegisterProviderOwner`; confirm with the provider-owner query before accepting telemetry.
2. Set `governance.sorafs_telemetry.submitters` and any `per_provider_submitters` overrides; keep `require_nonce=true` unless running a controlled replay drill.
3. Delegate `CanOperateSorafsRepair` to repair worker accounts before enabling automation, and rotate by revoking the permission plus reissuing worker keys (no admin-only bypass for repair actions).
4. Register `sorafs_moderation_operator`, grant it only to reviewed canonical operator accounts, and confirm unsigned quarantine calls return `401` while signed non-operator calls return `403`.
5. Confirm submitters are funded with the configured SoraFS public pin-fee asset and that `governance.sorafs_pin_fee_*` points at the expected treasury before opening storage ingest.
6. Enable `torii.soranet_privacy_ingest` only after populating `tokens` and `allow_cidrs`; rotate credentials by reloading the config and watch `soranet_privacy_ingest_reject_total` for namespace/token rejects.
7. Verify ingress with a signed sample request (e.g., `curl -H "X-SoraNet-Privacy-Token: privacy-prod-token" …/v1/soranet/privacy/event`) and confirm the endpoint returns `202 Accepted`.
8. Monitor `soranet_privacy_ingest_reject_total{reason}`, `soranet_privacy_throttles_total`, and the SoraFS quota metrics to catch abuse early; keep the checklist alongside change tickets for token/allow-list rotations.
