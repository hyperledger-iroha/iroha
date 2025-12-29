---
lang: ar
direction: rtl
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# أسئلة التشغيل الشائعة لـ Norito-RPC

تُلخّص هذه الأسئلة مفاتيح rollout/rollback والتليمترية وآثار الإثبات المذكورة
في البنود **NRPC-2** و **NRPC-4** بحيث يمتلك المشغلون صفحة واحدة للرجوع أثناء
canary أو brownout أو تدريبات الحوادث. اعتبرها بوابة تسليم المناوبات؛ أما
الإجراءات التفصيلية فتوجد في `docs/source/torii/norito_rpc_rollout_plan.md` و
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. مفاتيح الإعداد

| المسار | الغرض | القيم/الملاحظات |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | مفتاح تشغيل/إيقاف صارم لـ Norito. | `true` يبقي معالجات HTTP مسجلة؛ `false` يعطلها بغض النظر عن المرحلة. |
| `torii.transport.norito_rpc.require_mtls` | فرض mTLS لنقاط Norito. | الافتراضي `true`. عطّل فقط في تجمعات staging المعزولة. |
| `torii.transport.norito_rpc.allowed_clients` | قائمة سماح لحسابات الخدمة/رموز API التي تستخدم Norito. | وفّر CIDR أو تجزئات رموز أو OIDC client IDs بحسب بيئتك. |
| `torii.transport.norito_rpc.stage` | مرحلة rollout المعلنة للـ SDKs. | `disabled` (رفض Norito وإجبار JSON)، `canary` (سماح للقائمة فقط + تليمترية معززة)، `ga` (الافتراضي لكل عميل موثّق). |
| `torii.preauth_scheme_limits.norito_rpc` | حدود التزامن والاندفاع لكل مخطط. | استخدم مفاتيح throttles الخاصة بـ HTTP/WS (مثل `max_in_flight`, `rate_per_sec`). رفع الحد دون تحديث Alertmanager يُفشل الحاجز الوقائي. |
| `transport.norito_rpc.*` في `docs/source/config/client_api.md` | Overrides جهة العميل (CLI/SDK discovery). | استخدم `cargo xtask client-api-config diff` لمراجعة التغييرات قبل دفعها إلى Torii. |

**تدفق brownout الموصى به**

1. اضبط `torii.transport.norito_rpc.stage=disabled`.
2. أبقِ `enabled=true` حتى تستمر الاختبارات/الإنذارات في تمرير handlers.
3. اضبط `torii.preauth_scheme_limits.norito_rpc.max_in_flight` إلى صفر عند الحاجة
   لإيقاف فوري (مثلاً أثناء انتظار انتشار الإعداد).
4. حدّث سجل المشغّل وأرفق digest الإعداد الجديد بتقرير المرحلة.

## 2. قوائم التشغيل

- **Canary / staging** — اتبع `docs/source/runbooks/torii_norito_rpc_canary.md`.
  يشير إلى المفاتيح نفسها ويذكر الأدلة التي تجمعها
  `scripts/run_norito_rpc_smoke.sh` + `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **ترقية الإنتاج** — نفّذ قالب تقرير المرحلة في
  `docs/source/torii/norito_rpc_stage_reports.md`. سجّل hash الإعداد، hash allowlist،
  digest حزمة smoke، hash تصدير Grafana، ومعرّف drill التنبيه.
- **Rollback** — أعد `stage` إلى `disabled` واحتفظ بالـ allowlist ووثّق التحويل في
  تقرير المرحلة وسجل الحادثة. بعد إصلاح السبب أعد تشغيل checklist canary قبل `stage=ga`.

## 3. التليمترية والتنبيهات

| الأصل | الموقع | ملاحظات |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | يتتبع معدل الطلبات، أكواد الأخطاء، أحجام الحمولة، فشل فك الترميز، ونسبة التبني. |
| Alerts | `dashboards/alerts/torii_norito_rpc_rules.yml` | بوابات `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, `NoritoRpcFallbackSpike`. |
| سكربت Chaos | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | يفشل CI إذا انحرفت عبارات التنبيه. شغّله بعد كل تغيير إعداد. |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | أدرج السجلات في حزمة الأدلة لكل ترقية. |

يجب تصدير اللوحات وإرفاقها بتذكرة الإصدار (`make docs-portal-dashboards` في CI)
حتى يتمكن on‑call من إعادة تشغيل القياسات دون الوصول إلى Grafana الإنتاجية.

## 4. أسئلة شائعة

**كيف أسمح لـ SDK جديد أثناء canary؟**  
أضف حساب الخدمة/الرمز إلى `torii.transport.norito_rpc.allowed_clients`، أعد
تحميل Torii، وسجّل التغيير في `docs/source/torii/norito_rpc_tracker.md` تحت NRPC-2R.
يجب على مالك الـ SDK التقاط تشغيل fixtures عبر `scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**ماذا لو فشل فك ترميز Norito أثناء rollout؟**  
ابقَ على `stage=canary` و`enabled=true` وافحص الفشل عبر `torii_norito_decode_failures_total`.
يمكن للـ SDK الرجوع إلى JSON عبر إزالة `Accept: application/x-norito`؛ سيستمر Torii
بتقديم JSON حتى يعود stage إلى `ga`.

**كيف أثبت أن البوابة تخدم البيان الصحيح؟**  
شغّل `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host <gateway-host>`
حتى تسجل الأداة رؤوس `Sora-Proof` مع digest إعداد Norito. أرفق خرج JSON بتقرير المرحلة.

**أين أسجل overrides الخاصة بالتنقيح؟**  
وثّق كل override مؤقت في عمود `Notes` بتقرير المرحلة وسجل تصحيح إعداد Norito ضمن
إدارة التغييرات. تنتهي هذه overrides تلقائياً في ملف الإعداد؛ هذه الفقرة تضمن
أن فريق المناوبة لا ينسى التنظيف بعد الحوادث.

لأي سؤال غير مغطى هنا، صعّد عبر قنوات canary المذكورة في
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 5. مقتطف ملاحظات الإصدار (متابعة OPS-NRPC)

يتطلب بند **OPS-NRPC** مقتطف ملاحظات إصدار جاهز حتى يعلن المشغلون rollout
Norito‑RPC بشكل متسق. انسخ الكتلة أدناه إلى منشور الإصدار التالي (واستبدل
المجالات بين الأقواس) وأرفق حزمة الإثبات الموضحة أدناه.

> **نقل Torii Norito-RPC** — تُقدَّم مغلفات Norito الآن إلى جانب JSON API. يتم
> شحن العلم `torii.transport.norito_rpc.stage` مضبوطاً على
> **[stage: disabled/canary/ga]** ويتبع قائمة rollout المرحلية في
> `docs/source/torii/norito_rpc_rollout_plan.md`. يمكن للمشغلين الانسحاب مؤقتاً
> عبر ضبط `torii.transport.norito_rpc.stage=disabled` مع إبقاء
> `torii.transport.norito_rpc.enabled=true`؛ وستعود SDKs إلى JSON تلقائياً.
> تظل لوحات القياس (`dashboards/grafana/torii_norito_rpc_observability.json`) و
> تدريبات التنبيه (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) إلزامية
> قبل رفع المرحلة، ويجب إرفاق آثار canary/smoke من
> `python/iroha_python/scripts/run_norito_rpc_smoke.sh` بتذكرة الإصدار.

قبل النشر:

1. استبدل **[stage: …]** بالمرحلة المعلنة في Torii.
2. اربط تذكرة الإصدار بأحدث تقرير مرحلة في `docs/source/torii/norito_rpc_stage_reports.md`.
3. ارفع صادرات Grafana/Alertmanager المذكورة أعلاه مع hashes حزمة smoke من
   `scripts/run_norito_rpc_smoke.sh`.

هذا المقتطف يحقق متطلبات OPS-NRPC دون إجبار قادة الحوادث على إعادة صياغة حالة
rollout في كل مرة.

</div>
