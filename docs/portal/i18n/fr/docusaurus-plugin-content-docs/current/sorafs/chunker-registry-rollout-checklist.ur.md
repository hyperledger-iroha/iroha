---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry-rollout-checklist
title: SoraFS chunker registry rollout چیک لسٹ
sidebar_label: Chunker rollout چیک لسٹ
description: chunker registry updates کے لیے قدم بہ قدم rollout پلان۔
---

:::note مستند ماخذ
:::

# SoraFS registry rollout چیک لسٹ

یہ چیک لسٹ نئے chunker profile یا provider admission bundle کو review سے production
تک promote کرنے کے لیے درکار مراحل کو capture کرتی ہے جب governance charter
ratify ہو چکا ہو۔

> **Scope:** تمام releases پر لاگو ہے جو
> `sorafs_manifest::chunker_registry`، provider admission envelopes، یا canonical
> fixture bundles (`fixtures/sorafs_chunker/*`) میں تبدیلی کریں۔

## 1. Pre-flight validation

1. fixtures دوبارہ generate کریں اور determinism verify کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ profile report) میں
   determinism hashes regenerated artifacts سے match کریں۔
3. یقینی بنائیں کہ `sorafs_manifest::chunker_registry`،
   `ensure_charter_compliance()` کے ساتھ compile ہوتا ہے، چلائیں:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. proposal dossier اپڈیٹ کریں:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Council minutes entry `docs/source/sorafs/council_minutes_*.md`
   - Determinism report

## 2. Governance sign-off

1. Tooling Working Group report اور proposal digest کو
   Sora Parliament Infrastructure Panel میں پیش کریں۔
2. approval details کو
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` میں ریکارڈ کریں۔
3. Parliament-signed envelope کو fixtures کے ساتھ publish کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. governance fetch helper کے ذریعے envelope accessible ہونے کی تصدیق کریں:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Staging rollout

ان steps کی تفصیلی walkthrough کے لیے [staging manifest playbook](./staging-manifest-playbook) دیکھیں۔

1. Torii کو `torii.sorafs` discovery enabled اور admission enforcement on
   (`enforce_admission = true`) کے ساتھ deploy کریں۔
2. approved provider admission envelopes کو staging registry directory میں push کریں
   جسے `torii.sorafs.discovery.admission.envelopes_dir` refer کرتا ہے۔
3. discovery API کے ذریعے provider adverts کی propagation verify کریں:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. governance headers کے ساتھ manifest/plan endpoints exercise کریں:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. telemetry dashboards (`torii_sorafs_*`) اور alert rules سے نئے profile کی
   رپورٹنگ بغیر errors کے confirm کریں۔

## 4. Production rollout

1. staging steps کو production Torii nodes پر repeat کریں۔
2. activation window (date/time, grace period, rollback plan) کو operator اور SDK
   channels پر announce کریں۔
3. release PR merge کریں جس میں شامل ہو:
   - updated fixtures اور envelope
   - documentation changes (charter references, determinism report)
   - roadmap/status refresh
4. release tag کریں اور signed artifacts کو provenance کے لیے archive کریں۔

## 5. Post-rollout audit

1. rollout کے 24h بعد final metrics (discovery counts, fetch success rate, error
   histograms) capture کریں۔
2. `status.md` کو مختصر summary اور determinism report کے link کے ساتھ update کریں۔
3. follow-up tasks (مثلاً اضافی profile authoring guidance) کو `roadmap.md` میں درج کریں۔
