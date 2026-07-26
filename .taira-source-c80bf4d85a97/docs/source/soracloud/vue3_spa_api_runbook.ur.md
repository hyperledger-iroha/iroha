<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API رن بک

یہ رن بک پیداوار پر مبنی تعیناتی اور آپریشنز کا احاطہ کرتی ہے:

- ایک Vue3 جامد سائٹ (`--template site`)؛ اور
- ایک Vue3 SPA + API سروس (`--template webapp`)،

SCR/IVM مفروضوں کے ساتھ Iroha 3 پر Soracloud کنٹرول پلین APIs کا استعمال کرتے ہوئے (نہیں
WASM رن ٹائم انحصار اور Docker انحصار نہیں)۔

## 1. ٹیمپلیٹ پروجیکٹس بنائیں

جامد سائٹ کا سہارا:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API اسکافولڈ:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

ہر آؤٹ پٹ ڈائرکٹری میں شامل ہیں:

- `container_manifest.json`
- `service_manifest.json`
- ٹیمپلیٹ سورس فائلیں `site/` یا `webapp/` کے تحت

## 2. ایپلیکیشن کے نمونے بنائیں

جامد سائٹ:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA فرنٹ اینڈ + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. فرنٹ اینڈ اثاثوں کو پیکیج اور شائع کریں۔

SoraFS کے ذریعے جامد ہوسٹنگ کے لیے:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA فرنٹ اینڈ کے لیے:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. براہ راست سوراکاؤڈ کنٹرول طیارے میں تعینات کریں۔

جامد سائٹ سروس تعینات کریں:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API سروس تعینات کریں:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

روٹ بائنڈنگ اور رول آؤٹ حالت کی توثیق کریں:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

متوقع کنٹرول پلین چیکس:

- `control_plane.services[].latest_revision.route_host` سیٹ
- `control_plane.services[].latest_revision.route_path_prefix` سیٹ (`/` یا `/api`)
- اپ گریڈ کے فوراً بعد `control_plane.services[].active_rollout` موجود ہے۔

## 5. ہیلتھ گیٹڈ رول آؤٹ کے ساتھ اپ گریڈ کریں۔

1. سروس مینی فیسٹ میں `service_version` کو ٹکرائیں۔
2. اپ گریڈ چلائیں:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. صحت کی جانچ کے بعد رول آؤٹ کو فروغ دیں:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. اگر صحت خراب ہو جائے تو غیر صحت بخش رپورٹ کریں:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

جب غیر صحت بخش رپورٹس پالیسی کی حد تک پہنچ جاتی ہیں، Soracloud خود بخود رول ہوجاتا ہے۔
واپس بیس لائن پر نظرثانی کریں اور رول بیک آڈٹ واقعات کو ریکارڈ کریں۔

## 6. دستی رول بیک اور واقعہ کا جواب

پچھلے ورژن پر واپسی:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

تصدیق کے لیے اسٹیٹس آؤٹ پٹ کا استعمال کریں:

- `current_version` واپس کر دیا گیا۔
- `audit_event_count` اضافہ ہوا۔
- `active_rollout` صاف ہو گیا۔
- `last_rollout.stage` خودکار رول بیکس کے لیے `RolledBack` ہے

## 7. آپریشنز چیک لسٹ

- ٹیمپلیٹ سے تیار کردہ مینی فیسٹس کو ورژن کنٹرول میں رکھیں۔
- ٹریس ایبلٹی کو محفوظ رکھنے کے لیے ہر رول آؤٹ مرحلہ کے لیے `governance_tx_hash` ریکارڈ کریں۔
- علاج `service_health`, `routing`, `resource_pressure`، اور
  `failed_admissions` رول آؤٹ گیٹ ان پٹس کے بطور۔
- براہ راست مکمل کٹوتی کے بجائے کینری فیصد اور واضح فروغ کا استعمال کریں۔
  صارف کا سامنا کرنے والی خدمات کے لیے اپ گریڈ۔
- میں سیشن/تصدیق اور دستخطی توثیق کے رویے کی توثیق کریں۔
  پیداوار سے پہلے `webapp/api/server.mjs`۔