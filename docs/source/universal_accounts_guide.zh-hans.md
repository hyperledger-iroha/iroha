---
lang: zh-hans
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5d525863066feb78b2668d766816ed9404cef6e8159dea85db0fc8c1bcec9d01
source_last_modified: "2026-01-30T18:06:03.658066+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Universal Account Guide

This guide distils the UAID (Universal Account ID) rollout requirements from
the Nexus roadmap and packages them into an operator + SDK focused walkthrough.
It covers UAID derivation, portfolio/manifest inspection, regulator templates,
and the evidence that must accompany every `iroha app space-directory manifest
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID quick reference

- UAIDs are `uaid:<hex>` literals where `<hex>` is a Blake2b-256 digest whose
  LSB is set to `1`. The canonical type lives in
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Account records (`Account` and `AccountDetails`) now carry an optional `uaid`
  field so applications can learn the identifier without bespoke hashing.
- Space Directory maintains a `World::uaid_dataspaces` map that ties each UAID
  to the dataspace accounts referenced by active manifests. Torii reuses that
  map for the `/portfolio` and `/uaids/*` APIs.
- `POST /v2/accounts/onboard` publishes a default Space Directory manifest for
  the global dataspace when none exists, so the UAID is immediately bound.
  Onboarding authorities must hold `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- All SDKs expose helpers for canonicalising UAID literals (e.g.,
  `UaidLiteral` in the Android SDK). The helpers accept raw 64-hex digests
  (LSB=1) or `uaid:<hex>` literals and re-use the same Norito codecs so the
  digest cannot drift across languages.

## 2. Deriving and verifying UAIDs

There are three supported ways to obtain a UAID:

1. **Read it from world state or SDK models.** Any `Account`/`AccountDetails`
   payload queried via Torii now has the `uaid` field populated when the
   participant opted into universal accounts.
2. **Query the UAID registries.** Torii exposes
   `GET /v2/space-directory/uaids/{uaid}` which returns the dataspace bindings
   and manifest metadata the Space Directory host persists (see
   `docs/space-directory.md` §3 for payload samples).
3. **Derive it deterministically.** When bootstrapping new UAIDs offline, hash
   the canonical participant seed with Blake2b-256 and prefix the result with
   `uaid:`. The snippet below mirrors the helper documented in
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```

Always store the literal in lower case and normalise whitespace before hashing.
CLI helpers such as `iroha app space-directory manifest scaffold` and the Android
`UaidLiteral` parser apply the same trimming rules so governance reviews can
cross-check values without ad hoc scripts.

## 3. Inspecting UAID holdings and manifests

The deterministic portfolio aggregator in `iroha_core::nexus::portfolio`
surfaces every asset/dataspace pair that references the UAID. Operators and SDKs
can consume the data through the following surfaces:

| Surface | Usage |
|---------|-------|
| `GET /v2/accounts/{uaid}/portfolio` | Returns dataspace → asset → balance summaries; described in `docs/source/torii/portfolio_api.md`. |
| `GET /v2/space-directory/uaids/{uaid}` | Lists dataspace IDs + account literals tied to the UAID. |
| `GET /v2/space-directory/uaids/{uaid}/manifests` | Provides the full `AssetPermissionManifest` history for audits. |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI shortcut that wraps the bindings endpoint and optionally writes the JSON to disk (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Fetches the manifest JSON bundle for evidence packs. |

Example CLI session (Torii URL configured via `torii_api_url` in `iroha.json`):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Store the JSON snapshots alongside the manifest hash used during reviews; the
Space Directory watcher rebuilds the `uaid_dataspaces` map whenever manifests
activate, expire, or revoke, so these snapshots are the fastest way to prove
what bindings were active at a given epoch.

## 4. Publishing capability manifests with evidence

Use the CLI flow below whenever a new allowance is rolled out. Each step must
land in the evidence bundle recorded for governance sign-off.

1. **Encode the manifest JSON** so reviewers see the deterministic hash before
   submission:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Publish the allowance** using either the Norito payload (`--manifest`) or
   the JSON description (`--manifest-json`). Record the Torii/CLI receipt plus
   the `PublishSpaceDirectoryManifest` instruction hash:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture SpaceDirectoryEvent evidence.** Subscribe to
   `SpaceDirectoryEvent::ManifestActivated` and include the event payload in
   the bundle so auditors can confirm when the change landed.

4. **Generate an audit bundle** tying the manifest to its dataspace profile and
   telemetry hooks:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Verify bindings via Torii** (`bindings fetch` and `manifests fetch`) and
   archive those JSON files with the hash + bundle above.

Evidence checklist:

- [ ] Manifest hash (`*.manifest.hash`) signed by the change approver.
- [ ] CLI/Torii receipt for the publish call (stdout or `--json-out` artefact).
- [ ] `SpaceDirectoryEvent` payload proving activation.
- [ ] Audit bundle directory with dataspace profile, hooks, and manifest copy.
- [ ] Bindings + manifest snapshots fetched from Torii post-activation.

This mirrors the requirements in `docs/space-directory.md` §3.2 while giving SDK
owners a single page to point to during release reviews.

## 5. Regulator/regional manifest templates

Use the in-repo fixtures as starting points when crafting capability manifests
for regulators or regional supervisors. They demonstrate how to scope allow/deny
rules and explain the policy notes reviewers expect.

| Fixture | Purpose | Highlights |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB audit feed. | Read-only allowances for `compliance.audit::{stream_reports, request_snapshot}` with deny-wins on retail transfers to keep regulator UAIDs passive. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA supervision lane. | Adds a capped `cbdc.supervision.issue_stop_order` allowance (PerDay window + `max_amount`) and an explicit deny on `force_liquidation` to enforce dual controls. |

When cloning these fixtures, update:

1. `uaid` and `dataspace` ids to match the participant and lane you’re enabling.
2. `activation_epoch`/`expiry_epoch` windows based on the governance schedule.
3. `notes` fields with the regulator’s policy references (MiCA article, JFSA
   circular, etc.).
4. Allowance windows (`PerSlot`, `PerMinute`, `PerDay`) and optional
   `max_amount` caps so SDKs enforce the same limits as the host.

## 6. Migration notes for SDK consumers

Existing SDK integrations that referenced per-domain account IDs must migrate to
the UAID-centric surfaces described above. Use this checklist during upgrades:

  account ids. For Rust/JS/Swift/Android this means upgrading to the latest
  workspace crates or regenerating Norito bindings.
- **API calls:** Replace domain-scoped portfolio queries with
  `GET /v2/accounts/{uaid}/portfolio` and the manifest/bindings endpoints.
  `GET /v2/accounts/{uaid}/portfolio` accepts an optional `asset_id` query
  parameter when wallets only need a single asset instance. Client helpers such
  as `ToriiClient.getUaidPortfolio` (JS) and the Android
  `SpaceDirectoryClient` already wrap these routes; prefer them over bespoke
  HTTP code.
- **Caching & telemetry:** Cache entries by UAID + dataspace instead of raw
  account ids, and emit telemetry showing the UAID literal so operations can
  line up logs with Space Directory evidence.
- **Error handling:** New endpoints return the strict UAID parsing errors
  documented in `docs/source/torii/portfolio_api.md`; surface those codes
  verbatim so support teams can triage issues without repro steps.
- **Testing:** Wire the fixtures mentioned above (plus your own UAID manifests)
  into SDK test suites to prove Norito round-trips and manifest evaluations
  match the host implementation.

## 7. References

- `docs/space-directory.md` — operator playbook with deeper lifecycle detail.
- `docs/source/torii/portfolio_api.md` — REST schema for UAID portfolio and
  manifest endpoints.
- `crates/iroha_cli/src/space_directory.rs` — CLI implementation referenced in
  this guide.
- `fixtures/space_directory/capability/*.manifest.json` — regulator, retail, and
  CBDC manifest templates ready for cloning.
