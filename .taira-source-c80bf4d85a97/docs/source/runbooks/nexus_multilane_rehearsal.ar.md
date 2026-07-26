---
lang: ar
direction: rtl
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# دليل بروفة إطلاق Nexus متعددة المسارات

هذا الدليل يوجّه بروفة Phase B4 لنِكسَس. يتحقق من أن حزمة `iroha_config`
المعتمدة من الحوكمة وبيان genesis متعدد المسارات يعملان بشكل حتمي عبر
التليمترية، التوجيه، وتدريبات الرجوع.

## النطاق

- تشغيل المسارات الثلاثة في Nexus (`core`, `governance`, `zk`) مع إدخال Torii
  مختلط (معاملات، نشر عقود، إجراءات حوكمة) باستخدام seed الحمل الموقّع
  `NEXUS-REH-2026Q1`.
- التقاط artifacts القياس/التتبع المطلوبة لاعتماد B4 (Prometheus scrape،
  OTLP export، سجلات منظمة، تتبعات قبول Norito، مقاييس RBC).
- تنفيذ تدريب الرجوع `B4-RB-2026Q1` مباشرة بعد dry‑run والتأكد من إعادة تطبيق
  الملف أحادي المسار بشكل نظيف.

## المتطلبات المسبقة

1. يعكس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` موافقة
   GOV-2026-03-19 (البيانات الموقعة + أحرف المراجعين).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, مع
   تضمين `nexus.enabled = true`) و`defaults/nexus/genesis.json` يطابقان hashes
   المعتمدة؛ ويُظهر `kagami genesis bootstrap --profile nexus` نفس الـ digest
   المسجل في المتعقب.
3. يتطابق كتالوج المسارات مع مخطط الثلاثة مسارات المعتمد؛ يجب أن يُظهر
   `irohad --sora --config defaults/nexus/config.toml` لافتة Nexus router.
4. اختبارات CI متعددة المسارات خضراء: `ci/check_nexus_multilane_pipeline.sh`
   (يشغّل `integration_tests/tests/nexus/multilane_pipeline.rs` عبر
   `.github/workflows/integration_tests_multilane.yml`) و
   `ci/check_nexus_multilane.sh` (تغطية router) ينجحان ليبقى ملف Nexus جاهزاً
   للتعدد ( `nexus.enabled = true`، hashes كتالوج Sora سليمة، التخزين تحت
   `blocks/lane_{id:03}_{slug}` وسجلات الدمج مُهيّأة). التقط digests الآثار في
   المتعقب عند تغيير حزمة defaults.
5. لوحات القياس والتنبيهات لمقاييس Nexus مستوردة في مجلد Grafana الخاص بالبروفة؛
   مسارات التنبيه تشير إلى خدمة PagerDuty الخاصة بالبروفة.
6. مسارات Torii SDK مُهيّأة وفق جدول سياسة التوجيه ويمكنها إعادة تشغيل حمل البروفة محلياً.

## نظرة على الجدول الزمني

| المرحلة | نافذة الهدف | المالك | معيار الخروج |
|-------|-------------|----------|---------------|
| التحضير | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | نشر seed، تجهيز اللوحات، تهيئة عقد البروفة. |
| تجميد staging | Apr 8 2026 18:00 UTC | @release-eng | إعادة التحقق من hashes config/genesis؛ إرسال إشعار التجميد. |
| التنفيذ | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | اكتمال checklist دون حوادث مانعة؛ أرشفة حزمة القياس. |
| تدريب الرجوع | فوراً بعد التنفيذ | @sre-core | اكتمال `B4-RB-2026Q1`؛ التقاط تليمترية الرجوع. |
| الاسترجاع | حتى Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | نشر وثيقة الدروس والمتعقب. |

## قائمة تنفيذ (Apr 9 2026 15:00 UTC)

1. **إثبات الإعدادات** — `iroha_cli config show --actual` على كل عقدة؛ تحقق من
   تطابق hashes مع إدخال المتعقب.
2. **تهيئة المسارات** — أعد تشغيل seed لمدة فتحتين وتأكد أن
   `nexus_lane_state_total` يظهر نشاطاً في المسارات الثلاثة.
3. **التقاط القياس** — سجّل لقطات Prometheus `/metrics`، عينات OTLP، سجلات Torii
   المنظمة (لكل lane/dataspace)، ومقاييس RBC.
4. **خطافات الحوكمة** — نفّذ مجموعة معاملات الحوكمة وتحقق من التوجيه والعلامات.
5. **تدريب حادثة** — حاكِ تشبع مسار حسب الخطة؛ تحقق من إطلاق التنبيهات وتوثيق الاستجابة.
6. **تدريب الرجوع `B4-RB-2026Q1`** — طبّق ملف المسار الواحد، أعد تنفيذ checklist الرجوع،
   اجمع أدلة القياس، ثم أعد تطبيق حزمة Nexus.
7. **رفع artifacts** — ارفع حزمة القياس وتتبعات Torii وسجل التدريب إلى bucket
   الإثباتات، واربطها في `docs/source/nexus_transition_notes.md`.
8. **بيان/تحقق** — شغّل `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` لإنتاج `telemetry_manifest.json`
   + `.sha256` ثم أرفق المانيفست بمدخل المتعقب. تقوم الأداة بتطبيع حدود الفتحات
   (كأعداد صحيحة في البيان) وتفشل سريعاً إذا غاب أي مؤشر لضمان حتمية الأدلة.

## المخرجات

- قائمة بروفة موقّعة + سجل تدريب الحوادث.
- حزمة التليمترية (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Manifest التليمترية + digest من أداة التحقق.
- وثيقة مراجعة تلخص الحواجز والمعالجات والمسؤولين.

## ملخص التنفيذ — Apr 9 2026

- نُفّذت البروفة من 15:00 UTC إلى 16:12 UTC مع seed `NEXUS-REH-2026Q1`؛ حافظت
  المسارات الثلاثة على ~2.4k TEU لكل فتحة وأظهر `nexus_lane_state_total`
  توازن envelopes.
- أُرشفت حزمة التليمترية في `artifacts/nexus/rehearsals/2026q1/` (تتضمن
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, سجل الحادثة
  وأدلة الرجوع). سُجلت checksums في
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- اكتمل تدريب الرجوع `B4-RB-2026Q1` الساعة 16:18 UTC؛ أُعيد تطبيق ملف المسار
  الواحد خلال 6m42s دون تعطل مسارات، ثم أُعيد تفعيل حزمة Nexus بعد تأكيد القياس.
- حادثة تشبع المسار المُحقنة عند الفتحة 842 (forced headroom clamp) أطلقت التنبيهات
  المتوقعة؛ أنهى playbook المعالجة الصفحة خلال 11m مع توثيق خط الزمن في PagerDuty.
- لم تُرصد عوائق؛ العناصر اللاحقة (أتمتة تسجيل TEU headroom، سكربت تحقق حزمة التليمترية)
  متتبعة في مراجعة Apr 15.

## التصعيد

- الحوادث المانعة أو تراجعات القياس توقف البروفة وتتطلب تصعيداً للحوكمة خلال 4 ساعات عمل.
- أي انحراف عن حزمة config/genesis المعتمدة يستلزم إعادة البروفة بعد إعادة الموافقة.

## التحقق من حزمة التليمترية (مكتمل)

نفّذ `scripts/telemetry/validate_nexus_telemetry_pack.py` بعد كل بروفة لإثبات
أن حزمة التليمترية تحتوي على الآثار القياسية (Prometheus export، OTLP NDJSON،
سجلات Torii المنظمة، سجل الرجوع) مع التقاط digests SHA‑256. تكتب الأداة
`telemetry_manifest.json` وملف `.sha256` المطابق ليتمكن فريق الحوكمة من الاستشهاد
بـ hashes مباشرة في حزمة الاسترجاع.

لبروفة Apr 9 2026 يقع البيان المُتحقق منه بجانب الآثار في
`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` مع digest في
`telemetry_manifest.json.sha256`. أرفق الملفين بمدخل المتعقب عند النشر.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

استخدم `--require-slot-range` / `--require-workload-seed` في CI لمنع رفع الحزم
التي تنسى هذه التعليقات. استخدم `--expected <name>` لإضافة artifacts إضافية
(مثل إيصالات DA) إذا تطلبت خطة البروفة ذلك.

</div>
