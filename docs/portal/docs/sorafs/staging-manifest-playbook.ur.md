---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 16ab39ab208c5674e8ed2e0d98f630d81dfa897849d9e3a124aee2b10c90f2b8
source_last_modified: "2025-11-04T13:21:05.750215+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: staging-manifest-playbook
title: SoraFS staging manifest playbook
sidebar_label: SoraFS staging manifest playbook
description: Torii کی staging deployments پر Parliament-ratified chunker profile فعال کرنے کے لیے checklist۔
---

:::note مستند ماخذ
:::

## Overview

یہ playbook staging Torii deployment پر Parliament-ratified chunker profile فعال کرنے کے مراحل بیان کرتا ہے تاکہ تبدیلی کو production میں promote کرنے سے پہلے تصدیق ہو سکے۔ یہ فرض کرتا ہے کہ SoraFS governance charter ratify ہو چکا ہے اور canonical fixtures repository میں موجود ہیں۔

## 1. Prerequisites

1. Canonical fixtures اور signatures sync کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. admission envelopes کی وہ directory تیار کریں جسے Torii startup پر پڑھے گا (example path): `/var/lib/iroha/admission/sorafs`.
3. یقینی بنائیں کہ Torii config discovery cache اور admission enforcement کو enable کرتی ہے:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Admission envelopes publish کریں

1. approved provider admission envelopes کو `torii.sorafs.discovery.admission.envelopes_dir` کے directory میں copy کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii restart کریں (یا اگر loader hot reload کے ساتھ wrap ہے تو SIGHUP بھیجیں).
3. admission messages کے لیے logs tail کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Discovery propagation validate کریں

1. provider pipeline سے بنے ہوئے signed provider advert payload (Norito bytes) کو post کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. discovery endpoint query کریں اور confirm کریں کہ advert canonical aliases کے ساتھ نظر آتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   یقینی بنائیں کہ `profile_aliases` میں `"sorafs.sf1@1.0.0"` پہلی entry کے طور پر شامل ہو۔

## 4. Manifest اور plan endpoints exercise کریں

1. manifest metadata fetch کریں (اگر admission enforce ہے تو stream token درکار ہوگا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON output inspect کریں اور verify کریں:
   - `chunk_profile_handle`، `sorafs.sf1@1.0.0` ہو۔
   - `manifest_digest_hex` determinism report سے match کرے۔
   - `chunk_digests_blake3` regenerated fixtures سے align ہوں۔

## 5. Telemetry checks

- تصدیق کریں کہ Prometheus نئی profile metrics expose کر رہا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- dashboards کو staging provider expected alias کے تحت دکھانا چاہیے اور profile فعال ہونے پر brownout counters صفر رہنے چاہییں۔

## 6. Rollout readiness

1. URLs، manifest ID، اور telemetry snapshot کے ساتھ مختصر رپورٹ بنائیں۔
2. رپورٹ کو Nexus rollout channel میں planned production activation window کے ساتھ شیئر کریں۔
3. stakeholders کے sign off کے بعد production checklist پر جائیں (Section 4 in `chunker_registry_rollout_checklist.md`).

اس playbook کو اپ ڈیٹ رکھنا یقینی بناتا ہے کہ ہر chunker/admission rollout staging اور production میں ایک جیسے deterministic steps پر چلتا ہے۔
