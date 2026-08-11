# SoraFS/SoraNet Authz Runbook

This note summarises the authorization and abuse controls around SoraFS control-plane actions and the SoraNet privacy ingest endpoints so operators can provision tokens, bind providers, and rotate credentials without guesswork.

## Surfaces and tokens

- `RegisterPinManifest` is a paid public operation for any authenticated
  transaction account on the universal lane; there is no general pin
  permission token. Submission charges deterministic global/per-account count
  and byte quotas, collects the SoraFS public pin fee into the governance
  treasury, records fee metadata, and derives `submitted_epoch` from consensus
  time. A manifest activates immediately only when council approval is not
  required; otherwise it stays pending without a public alias or automatic
  replication order.
- `ApprovePinManifest` and `RetirePinManifest` do not use broad permission
  tokens or accept caller-selected epochs. Any authenticated account may relay
  the bounded threshold council envelope for a pending approval; the envelope,
  not the relayer, supplies governance authority. Retirement is restricted to
  the authenticated submitter. Core derives both event epochs from consensus
  time and releases pin quota on retirement. Alias attachment/binding keeps
  `CanBindSorafsAlias`. Other capacity, replication, pricing, and provider
  credit instructions retain their dedicated scoped authorization.
- Provider→account bindings must be present before issuing replication orders or submitting capacity telemetry. Configuration may seed them only before genesis. After genesis, submit an exact `ProposeSorafsProviderGovernance` establish/rebind/remove action and enact it through the native Parliament referendum; the direct `RegisterProviderOwner`/`UnregisterProviderOwner` surfaces always reject. Rebind and removal are compare-and-set operations and refuse live capacity or reserve state.
- A capacity declaration is admitted only when its exact governed owner has enough unslashed bond in an owner-funded native reserve partition to cover both the declared stake and the credit projection's required bond. The active reserve asset is protocol custody: ordinary transfer, burn, custody-account removal, and backing-definition removal reject; only an exact pending owner-requested withdrawal approved by the reserve decision authority can debit it. Outstanding treasury-funded credit principal is excluded, and `bonded + slashed` must equal the remaining owner-funded custody; withdrawals preserve the slash lien and update only the unslashed projection. `UpsertProviderCredit` is not a funding instruction and cannot mint collateral or erase slash history.
- Repair command endpoints accept exactly one caller-signed Iroha transaction
  containing the route-specific native instruction. Claims, renewals,
  completion, failure, and escalation require the transaction authority to
  hold `CanOperateSorafsRepair { provider_id }`; renew/terminal actions must
  also match the exact committed task revision, active lease owner, and lease
  generation. Provider owners may delegate and revoke the scoped permission
  with `GrantPermission`/`RevokePermission`. Deleted
  `RepairWorkerSignaturePayloadV1` bodies are not accepted as a compatibility
  format, and Torii never injects an authority into an unsigned request.
- V1 exposes no public storage-ingest API. `POST /v1/sorafs/pin/register` accepts only a canonical caller-signed transaction; after finality, a provider-internal durable outbox may ingest the payload only when the approved manifest, exact finalized height/hash, configured provider identity, and committed replication assignment all agree. Idempotency and dead-letter identities are derived from those committed fields, never from caller-selected HTTP metadata.
- Local moderation quarantine review, release, and encrypted object store/read endpoints require canonical request signatures from accounts holding the `sorafs_moderation_operator` role. Keep this as a dedicated empty role for the Torii API gate; do not attach broad ledger permissions to it unless a separate governance change requires them.
- SoraNet privacy ingest endpoints (`/v1/soranet/privacy/{event,share}`) require `X-SoraNet-Privacy-Token` (or `X-API-Token`), a non-empty CIDR allow-list, and the token/burst limits under `torii.soranet_privacy_ingest`; requests outside the namespace or over budget are rejected before metrics ingestion.

## Telemetry submitters and provider overrides

- `governance.sorafs_telemetry.require_submitter` and `require_nonce` default to `true`, forcing telemetry windows to come from authorised accounts with replay protection. When `require_nonce=false`, windows without a nonce are accepted but any provided nonces are still checked for replay.
- `submitters` defines the global allow-list; `per_provider_submitters` overrides it for specific providers when the submitting account differs from the default.
- Capacity telemetry still enforces provider ownership, window hygiene, and nonce replay detection; spoofed or mismatched owners are rejected with labelled telemetry.

## Torii ingress guards

- Storage ingest is not an ingress route in V1. Reject any deployment or generated OpenAPI/catalog that reintroduces a public POST upload path. Only the supervised provider worker consumes finalized ledger state and its durable outbox; ordinary HTTP traffic cannot alter storage bytes, quota reservations, or provider-keyed metadata.
- `torii.soranet_privacy_ingest`: disabled by default; enabling requires a token list and CIDR scope (empty list denies). The rate limiter uses `rate_per_sec`/`burst`, keyed by token/IP, and emits `soranet_privacy_ingest_reject_total{endpoint,reason}` on rejects.
- Sample configuration:

```toml
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
    --config /etc/iroha/config.toml
  ```
  Registration is a paid public operation for the authenticated account in the
  client config; it does not require a general pin permission token. Core
  derives the submission epoch from block consensus time and enforces the
  global/per-account count and byte quotas. The retired client epoch flag is
  not accepted.
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

1. Seed provider owners before genesis or enact an exact `SorafsProviderGovernanceActionV1` through Parliament; confirm the finalized owner query and owner-funded reserve balance before accepting capacity or telemetry.
2. Set `governance.sorafs_telemetry.submitters` and any `per_provider_submitters` overrides; keep `require_nonce=true` unless running a controlled replay drill.
3. Delegate `CanOperateSorafsRepair` to repair worker accounts before enabling automation, and rotate by revoking the permission plus reissuing worker keys (no admin-only bypass for repair actions).
4. Register `sorafs_moderation_operator`, grant it only to reviewed canonical operator accounts, and confirm unsigned quarantine calls return `401` while signed non-operator calls return `403`.
5. Confirm submitters are funded with the configured SoraFS public pin-fee asset and that `governance.sorafs_pin_fee_*` points at the expected treasury before opening storage ingest.
6. Enable `torii.soranet_privacy_ingest` only after populating `tokens` and `allow_cidrs`; rotate credentials by reloading the config and watch `soranet_privacy_ingest_reject_total` for namespace/token rejects.
7. Verify ingress with a signed sample request (e.g., `curl -H "X-SoraNet-Privacy-Token: privacy-prod-token" …/v1/soranet/privacy/event`) and confirm the endpoint returns `202 Accepted`.
8. Monitor `soranet_privacy_ingest_reject_total{reason}`,
   `soranet_privacy_throttles_total`, and the SoraFS operational gauges to catch
   abuse early. Use finalized `PinManifestPageV1.charged_usage`, not sampled
   Prometheus inventory, as the authoritative retained-record/live-content
   charge; keep
   the checklist alongside change tickets for token/allow-list rotations.
