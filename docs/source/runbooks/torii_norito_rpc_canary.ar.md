---
lang: ar
direction: rtl
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# دليل Canary لـ Torii Norito-RPC (NRPC-2C)

يُطبّق هذا الدليل خطة **NRPC-2** عبر شرح كيفية ترقية نقل Norito‑RPC من تحقق
المختبر في staging إلى مرحلة “canary” في الإنتاج. يجب قراءته إلى جانب:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (عقد البروتوكول)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## الأدوار والمدخلات

| الدور | المسؤولية |
|------|----------------|
| Torii Platform TL | يوافق على فروقات الإعداد ويوقع اختبارات smoke. |
| NetOps | يطبق تغييرات ingress/envoy ويراقب صحة تجمع canary. |
| مسؤول الرصد | يتحقق من اللوحات/التنبيهات ويجمع الأدلة. |
| Platform Ops | يدير تذكرة التغيير، ينسق تدريب الرجوع، ويحدث المتعقبات. |

الآثار المطلوبة:

- أحدث رقعة `iroha_config` الخاصة بـ Norito مع `transport.norito_rpc.stage = "canary"` و
  تعبئة `transport.norito_rpc.allowed_clients`.
- مقطع إعداد Envoy/Nginx يحافظ على `Content-Type: application/x-norito` ويفرض
  ملف mTLS لعملاء canary (`defaults/torii_ingress_mtls.yaml`).
- قائمة سماح للرموز (YAML أو بيان Norito) لعملاء canary.
- رابط Grafana + رمز API لـ `dashboards/grafana/torii_norito_rpc_observability.json`.
- الوصول إلى harness smoke
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) وسكربت تدريب التنبيهات
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## قائمة فحص ما قبل التنفيذ

1. **تأكيد تجميد المواصفة.** تأكد من أن hash `docs/source/torii/nrpc_spec.md`
   يطابق آخر إصدار موقّع وأنه لا توجد PRs تمس رأس/تخطيط Norito.
2. **التحقق من الإعداد.** شغّل
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   للتأكد من تحليل `transport.norito_rpc.*` بشكل صحيح.
3. **حدود المخطط.** اضبط `torii.preauth_scheme_limits.norito_rpc` بشكل محافظ
   (مثلاً 25 اتصالاً متزامناً) حتى لا تطغى الاتصالات الثنائية على JSON.
4. **تجربة ingress.** طبّق رقعة Envoy في staging، نفّذ الاختبار السلبي
   (`cargo test -p iroha_torii -- norito_ingress`) وتأكد من رفض الرؤوس المزالة بـ HTTP 415.
5. **سلامة التليمترية.** في staging شغّل `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` وأرفق حزمة الأدلة الناتجة.
6. **جرد الرموز.** تأكد أن allowlist canary تضم على الأقل مشغّلين اثنين لكل منطقة؛
   واحفظ البيان في `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **التذاكر.** افتح تذكرة التغيير مع نافذة البدء/الانتهاء وخطة الرجوع وروابط
   هذا الدليل مع الأدلة.

## إجراء ترقية canary

1. **تطبيق رقعة الإعداد.**
   - طبّق فرق `iroha_config` (stage=`canary`، allowlist مملوءة، حدود المخطط محددة) عبر admission.
   - أعد تشغيل Torii أو نفّذ hot‑reload وتحقق من اعترافه بالرقعة عبر سجلات `torii.config.reload`.
2. **تحديث ingress.**
   - انشر إعداد Envoy/Nginx الذي يفعّل توجيه Norito/mTLS لتجمع canary.
   - تأكد أن ردود `curl -vk --cert <client.pem>` تتضمن رؤوس `X-Iroha-Error-Code` عند الحاجة.
3. **اختبارات smoke.**
   - نفّذ `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     من canary bastion. خزّن نصوص JSON + Norito في
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - سجّل hashes في `docs/source/torii/norito_rpc_stage_reports.md`.
4. **مراقبة التليمترية.**
   - راقب `torii_active_connections_total{scheme="norito_rpc"}` و
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` لمدة 30 دقيقة على الأقل.
   - صدّر لوحة Grafana عبر API وأرفقها بتذكرة التغيير.
5. **تدريب التنبيهات.**
   - شغّل `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` لحقن
     أظرف Norito تالفة؛ تأكد من تسجيل Alertmanager للحادثة الاصطناعية وإغلاقها تلقائياً.
6. **التقاط الأدلة.**
   - حدّث `docs/source/torii/norito_rpc_stage_reports.md` بالقيم التالية:
     - Digest الإعداد
     - Hash بيان allowlist
     - توقيت smoke test
     - Checksum تصدير Grafana
     - معرّف drill التنبيه
   - ارفع الآثار إلى `artifacts/norito_rpc/<YYYYMMDD>/`.

## المراقبة ومعايير الخروج

ابقَ في canary حتى تتحقق الشروط التالية لمدة ≥72 ساعة:

- معدل الأخطاء (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % دون
  ارتفاعات مستمرة في `torii_norito_decode_failures_total`.
- تكافؤ الكمون (`p95` Norito مقابل JSON) ضمن 10 %.
- لوحة التنبيهات هادئة باستثناء التدريبات المجدولة.
- مشغلو allowlist يرسلون تقارير تكافؤ بلا تعارضات مخطط.

وثّق الحالة اليومية في تذكرة التغيير واحفظ لقطات في
`docs/source/status/norito_rpc_canary_log.md` (إن وُجد).

## إجراء الرجوع

1. أعد `transport.norito_rpc.stage` إلى `"disabled"` وامسح `allowed_clients`؛ طبّق عبر admission.
2. أزل route/mTLS stanza من Envoy/Nginx وأعد تحميل البروكسي وتأكد من رفض اتصالات Norito الجديدة.
3. اسحب رموز canary (أو عطّل بيانات bearer) لإسقاط الجلسات القائمة.
4. راقب `torii_active_connections_total{scheme="norito_rpc"}` حتى يصل إلى الصفر.
5. أعد تشغيل harness smoke الخاص بـ JSON فقط لضمان الوظائف الأساسية.
6. أنشئ stub post‑mortem في `docs/source/postmortems/norito_rpc_rollback.md` خلال 24 ساعة
   وحدث تذكرة التغيير بملخص الأثر + المقاييس.

## إغلاق ما بعد canary

عند تحقق معايير الخروج:

1. حدّث `docs/source/torii/norito_rpc_stage_reports.md` بتوصية GA.
2. أضف إدخالاً في `status.md` يوجز نتائج canary وحزم الأدلة.
3. أخطر قادة SDK لتحويل fixtures الخاصة بـ staging إلى Norito لاختبارات التكافؤ.
4. جهّز رقعة إعداد GA (stage=`ga`، إزالة allowlist) وجدول الترقية وفق NRPC-2.

اتباع هذا الدليل يضمن جمع الأدلة بشكل متسق، وrollback حتمي، وتحقيق شروط قبول NRPC-2.

</div>
