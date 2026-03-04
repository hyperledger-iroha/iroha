---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6049b1c4fb42bbfbeaa7fa8f3549c5b7beac1a3e8baec45c0c0ce52f0c3baa2e
source_last_modified: "2025-11-14T09:52:13.533271+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: لوحات مراقبة Taikai
description: ملخص بوابة لوحات Grafana الخاصة بـ viewer/cache التي تدعم أدلة SN13-C.
---

تعتمد جاهزية Taikai routing-manifest (TRM) على لوحتين من Grafana وتنبيهاتها
المرافقة. تعكس هذه الصفحة أبرز النقاط من
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`, و
`dashboards/alerts/taikai_viewer_rules.yml` حتى يتمكن المراجعون من المتابعة دون
نسخ المستودع.

## لوحة viewer (`taikai_viewer.json`)

- **Live edge والكمون:** تعرض اللوحات مخططات الكمون p95/p99
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) لكل
  cluster/stream. راقب p99 > 900 ms أو drift > 1.5 s (يفعل تنبيه
  `TaikaiLiveEdgeDrift`).
- **أخطاء المقاطع:** تفصل `taikai_ingest_segment_errors_total{reason}` لكشف فشل
  decode، ومحاولات lineage replay، أو عدم تطابق manifest. أرفق لقطات شاشة مع
  حوادث SN13-C عندما يتجاوز هذا اللوح نطاق “warning”.
- **صحة viewer و CEK:** اللوحات المستندة إلى مقاييس `taikai_viewer_*` تتبع عمر
  تدوير CEK، ومزيج PQ guard، وعدد مرات إعادة التخزين المؤقت، وتجميع التنبيهات.
  يفرض لوح CEK SLA للتدوير التي تراجعها الحوكمة قبل اعتماد aliases جديدة.
- **لقطة تليمترية aliases:** جدول `/status → telemetry.taikai_alias_rotations`
  موجود مباشرة على اللوحة لكي يؤكد المشغلون digests الخاصة بالـ manifest قبل
  إرفاق أدلة الحوكمة.

## لوحة cache (`taikai_cache.json`)

- **ضغط الـ tier:** ترسم اللوحات `sorafs_taikai_cache_{hot,warm,cold}_occupancy` و
  `sorafs_taikai_cache_promotions_total`. استخدمها لمعرفة ما إذا كانت دورة TRM
  تضغط على tiers محددة.
- **رفض QoS:** يظهر `sorafs_taikai_qos_denied_total` عندما يؤدي ضغط الكاش إلى
  throttling؛ دوّن سجل drill كلما ابتعد المعدل عن الصفر.
- **استخدام egress:** يساعد على تأكيد أن مخارج SoraFS تواكب viewers الخاصة بـ Taikai
  عند تدوير نوافذ CMAF.

## التنبيهات والتقاط الأدلة

- قواعد paging موجودة في `dashboards/alerts/taikai_viewer_rules.yml` وتطابق
  اللوحات أعلاه واحدا لواحد (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`,
  `TaikaiCekRotationLag`, تحذيرات proof-health). تأكد من أن جميع clusters الإنتاجية
  موصولة بـ Alertmanager.
- يجب تخزين snapshots/لقطات الشاشة المأخوذة أثناء drills في
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` بجانب spool files و JSON `/status`.
  استخدم `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` لإضافة
  التنفيذ إلى drill log المشترك.
- عند تغيير dashboards، أدرج digest SHA-256 لملف JSON داخل وصف PR للبوابة لكي
  يتمكن المدققون من مطابقة مجلد Grafana المدار مع إصدار المستودع.

## قائمة تحقق حزمة الأدلة

تتوقع مراجعات SN13-C أن يرسل كل drill أو حادث نفس artefacts المذكورة في runbook
لـ Taikai anchor. التقطها بالترتيب التالي حتى تكون الحزمة جاهزة لمراجعة الحوكمة:

1. انسخ أحدث ملفات `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json`, و `taikai-lineage-*.json` من
   `config.da_ingest.manifest_store_dir/taikai/`. تثبت هذه artefacts من spool أي
   routing manifest (TRM) وأي نافذة lineage كانت نشطة. المساعد
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   ينسخ spool files، ويصدر hashes، وقد يوقع الملخص اختياريا.
2. سجل مخرجات `/v1/status` المصفاة إلى
   `.telemetry.taikai_alias_rotations[]` واحفظها بجوار spool files. يقارن
   المراجعون `manifest_digest_hex` وحدود النافذة مع حالة spool المنسوخة.
3. صدّر Prometheus snapshots للمقاييس المذكورة أعلاه وخذ لقطات شاشة للوحات
   viewer/cache مع فلاتر cluster/stream ذات الصلة ظاهرة. ضع JSON/CSV الخام
   واللقطات داخل مجلد artefacts.
4. أرفق IDs حوادث Alertmanager (إن وجدت) التي تشير إلى قواعد
   `dashboards/alerts/taikai_viewer_rules.yml` ودوّن ما إذا أغلقت تلقائيا عند
   زوال الحالة.

احفظ كل شيء تحت `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` حتى تتمكن تدقيقات
الـ drill ومراجعات الحوكمة SN13-C من الحصول على أرشيف واحد.

## وتيرة drills والتسجيل

- نفّذ Taikai anchor drill في أول ثلاثاء من كل شهر الساعة 15:00 UTC. يحافظ هذا
  الجدول على الأدلة محدثة قبل مزامنة الحوكمة SN13.
- بعد التقاط artefacts أعلاه، أضف التنفيذ إلى السجل المشترك باستخدام
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`. يصدر المساعد
  إدخال JSON المطلوب من `docs/source/sorafs/runbooks-index.md`.
- اربط artefacts المؤرشفة في إدخال runbook index وصعّد أي alerts فاشلة أو
  تراجعات dashboards خلال 48 ساعة عبر قناة Media Platform WG/SRE.
- احتفظ بمجموعة لقطات drill summary (الكمون، drift، الأخطاء، تدوير CEK، ضغط الكاش)
  بجانب spool bundle حتى يستطيع المشغلون إظهار سلوك dashboards أثناء التمرين.

ارجع إلى [Taikai Anchor Runbook](./taikai-anchor-runbook.md) للحصول على إجراء
Sev 1 الكامل وقائمة الأدلة. تلتقط هذه الصفحة فقط إرشادات dashboards التي
تتطلبها SN13-C قبل مغادرة 🈺.
