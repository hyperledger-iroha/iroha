<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API Runbook

يغطي دليل التشغيل هذا النشر والعمليات الموجهة نحو الإنتاج من أجل:

- موقع Vue3 الثابت (`--template site`)؛ و
- خدمة Vue3 SPA + API (`--template webapp`)،

استخدام واجهات برمجة التطبيقات لمستوى التحكم Soracloud على Iroha 3 مع افتراضات SCR/IVM (لا
تبعية وقت تشغيل WASM ولا توجد تبعية Docker).

## 1. إنشاء مشاريع نموذجية

سقالة الموقع الثابت:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

سقالة SPA + API:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

يتضمن كل دليل إخراج ما يلي:

-`container_manifest.json`
-`service_manifest.json`
- ملفات مصدر القالب ضمن `site/` أو `webapp/`

## 2. بناء عناصر التطبيق

موقع ثابت:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

واجهة SPA + واجهة برمجة التطبيقات:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. قم بتجميع أصول الواجهة الأمامية ونشرها

للاستضافة الثابتة عبر SoraFS:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

للواجهة الأمامية لـ SPA:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. قم بالنشر المباشر لطائرة التحكم Soracloud

نشر خدمة الموقع الثابت:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

نشر خدمة SPA + API:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

التحقق من صحة ربط المسار وحالة الطرح:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

فحوصات طائرة التحكم المتوقعة:

- مجموعة `control_plane.services[].latest_revision.route_host`
- مجموعة `control_plane.services[].latest_revision.route_path_prefix` (`/` أو `/api`)
- `control_plane.services[].active_rollout` موجود مباشرة بعد الترقية

## 5. قم بالترقية من خلال طرح بوابات صحية

1. اعثر على `service_version` في بيان الخدمة.
2. تشغيل الترقية:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. الترويج للطرح بعد إجراء فحوصات السلامة:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. إذا تدهورت الصحة، أبلغ عن عدم الصحة:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

عندما تصل التقارير غير الصحية إلى عتبة السياسة، يتم تشغيل Soracloud تلقائيًا
العودة إلى مراجعة خط الأساس وتسجيل أحداث تدقيق التراجع.

## 6. التراجع اليدوي والاستجابة للحوادث

العودة إلى الإصدار السابق:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

استخدم مخرجات الحالة للتأكيد:

- تم إرجاع `current_version`
- تمت زيادة `audit_event_count`
- تم مسح `active_rollout`
- `last_rollout.stage` هو `RolledBack` لعمليات التراجع التلقائية

## 7. قائمة مراجعة العمليات

- احتفظ بالبيانات التي تم إنشاؤها بواسطة القالب تحت التحكم في الإصدار.
- قم بتسجيل `governance_tx_hash` لكل خطوة من خطوات الطرح للحفاظ على إمكانية التتبع.
- علاج `service_health`، `routing`، `resource_pressure`، و
  `failed_admissions` كمدخلات لبوابة التشغيل.
- استخدم نسب الكناري والترويج الصريح بدلاً من القطع الكامل المباشر
  ترقيات للخدمات التي تواجه المستخدم.
- التحقق من صحة الجلسة/المصادقة وسلوك التحقق من التوقيع في
  `webapp/api/server.mjs` قبل الإنتاج.