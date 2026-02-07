---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تشغيل البوابة و DNS لـ SoraFS

هذه نسخة من البوابة تعكس دليل التشغيل القانوني باللغة الإنجليزية
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
احصل على عمليات الإنقاذ من تدفق عمل DNS والبوابة اللامركزية
حتى يتمكن قادة الشبكات والعمليات والوثائق من قراءة الورقة
الأتمتة قبل انطلاق المباراة 2025-03.

## Alcance y entregables

- وصلات DNS (SF-4) والبوابة (SF-5) لمعرفة الاشتقاق
  تحديد المضيفين، وإصدارات دليل الحلول، وأتمتة TLS/GAR
  والتقاط الأدلة.
- الحفاظ على مستلزمات انطلاق المباراة (جدول الأعمال، الدعوة، متتبع المساعدة، اللقطة
  de telemetría GAR) تمت مزامنته مع آخر تعيينات المالكين.
- إنتاج حزمة من المصنوعات القابلة للتدقيق لمراجعة الإدارة: ملاحظات
  قم بتحرير دليل المحللين وسجلات البوابة وإخراج الحزام
  من المطابقة واستئناف Docs/DevRel.

## الأدوار والمسؤوليات| مسار العمل | المسؤوليات | المصنوعات اليدوية المطلوبة |
|------------|-----------------------------------|-------|
| شبكات TL (مكدس DNS) | الحفاظ على خطة تحديد المضيفين، وتنفيذ إصدارات دليل RAD، ونشر مدخلات أجهزة القياس عن بعد من المحللين. | `artifacts/soradns_directory/<ts>/`، يختلف عن `docs/source/soradns/deterministic_hosts.md`، البيانات التعريفية RAD. |
| قائد أتمتة العمليات (البوابة) | قم بتشغيل تدريبات أتمتة TLS/ECH/GAR، وتصحيح `sorafs-gateway-probe`، وتحديث خطافات PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`، مسبار JSON، يتم إدخاله في `ops/drill-log.md`. |
| ضمان الجودة والأدوات WG | قم بتشغيل `ci/check_sorafs_gateway_conformance.sh`، وقم بمعالجة التركيبات، وحفظ حزم الشهادة الذاتية Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| مستندات / ديفريل | قم بتسجيل المحاضر، وتحديث القراءة المسبقة للتصميم + الملاحق، ونشر السيرة الذاتية للأدلة في هذه البوابة. | تم تحديث الأرشيف `docs/source/sorafs_gateway_dns_design_*.md` وملاحظات الطرح. |

## المدخلات والمتطلبات الأساسية- مواصفات محددات المضيفين (`docs/source/soradns/deterministic_hosts.md`) ذ
  مساعدة حل المصادقة (`docs/source/soradns/resolver_attestation_directory.md`).
- مصنوعات البوابة: دليل المشغل، مساعدي أتمتة TLS/ECH،
  دليل الوضع المباشر وتدفق الشهادة الذاتية `docs/source/sorafs_gateway_*`.
- الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh`، ومساعدي CI
  (`ci/check_sorafs_gateway_conformance.sh`، `ci/check_sorafs_gateway_probe.sh`).
- الأسرار: مفتاح إصدار GAR، وبيانات اعتماد ACME DNS/TLS، ومفتاح التوجيه PagerDuty،
  رمز مصادقة Torii للحصول على أدوات الحل.

## قائمة التحقق قبل فويلو

1. قم بتأكيد المساعدين وتحديث جدول الأعمال
   `docs/source/sorafs_gateway_dns_design_attendance.md` وقم بتدوير جدول الأعمال
   فيجنتي (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. قم بإعداد بذور المصنوعات اليدوية مثل ذلك
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` ذ
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. تركيبات التحديث (بيانات GAR واختبار RAD وحزم البوابة المطابقة) و
   تأكد من أن حالة `git submodule` تتزامن مع العلامة الأخيرة للتجربة.
4. التحقق من الأسرار (مفتاح الإصدار Ed25519، ملف حساب ACME، رمز PagerDuty)
   والذي يتزامن مع المجموع الاختباري للقبو.
5. اختبار وجود الدخان لأهداف القياس عن بعد (نقطة نهاية Pushgateway، لوحة GAR Grafana)
   قبل الحفر.

## خطوات البحث عن الأتمتة

### خريطة تحديد المضيفين وإصدار دليل RAD1. تنفيذ مساعد تحديد المضيفين مقابل مجموعة البيانات
   Propuesto وتأكيد أنه لا يوجد احترام للانجراف
   `docs/source/soradns/deterministic_hosts.md`.
2. إنشاء حزمة من أدوات الحل:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. قم بتسجيل معرف الدليل، SHA-256 ومسارات الطباعة داخل
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` وفي دقائق انطلاق المباراة.

### التقاط DNS عن بعد

- قم بإكمال سجلات الحلول الشفافة لمدة تزيد عن 10 دقائق من الاستخدام
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير مقاييس Pushgateway وحفظ اللقطات NDJSON جنبًا إلى جنب
  معرف دليل التشغيل.

### تدريبات على أتمتة البوابة

1. تشغيل الصوت TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. قم بتشغيل شريط المطابقة (`ci/check_sorafs_gateway_conformance.sh`) و
   مساعد الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) للتحديث
   حزمة الشهادات Norito.
3. التقط أحداث PagerDuty/Webhook لإظهار الأتمتة
   وظيفة من أقصى إلى أقصى.

### تعبئة الأدلة

- تحديث `ops/drill-log.md` مع الطوابع الزمنية والمشاركين وتجزئة التحقيقات.
- حماية المصنوعات من معرّف التشغيل ونشر استئناف تنفيذي
  في الدقائق من Docs/DevRel.
- قم بتضمين حزمة الأدلة في تذكرة الإدارة قبل المراجعة
  ديل انطلاقة.

## تسهيل الجلسة وجمع الأدلة- **خط زمن المشرف:**
  - T-24 h — إدارة البرامج تنشر السجل + لقطة من جدول الأعمال/المساعدة على `#nexus-steering`.
  - T-2 h — تقوم شبكة TL بتحديث لقطة القياس عن بعد GAR وتسجيل الدلتا في `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — تتحقق Ops Automation من إعداد المسبار وتكتب معرف التشغيل النشط في `artifacts/sorafs_gateway_dns/current`.
  - Durante la lamada — المشرف على مشاركة هذا الدليل وتعيينه للكتابة على قيد الحياة؛ يقوم Docs/DevRel بالتقاط عناصر العمل عبر الإنترنت.
- **دقائق الدقائق:** نسخ النمط
  `docs/source/sorafs_gateway_dns_design_minutes.md` (تم أيضًا الاستعانة به في الحزمة
  من البوابة) وقم بإنشاء مثيل كامل خلال الجلسة. تشمل القائمة
  المساعدون والقرارات وعناصر العمل وتجزئات الأدلة والنتائج المعلقة.
- **شحنة الأدلة:** قم بشراء الدليل `runbook_bundle/` للكتابة،
  إضافة PDF للدقائق المقدمة، وتسجيل التجزئات SHA-256 في الدقائق +
  جدول الأعمال وإشعارات بالاسم المستعار لمراجعي الإدارة عند الشحن
  aterricen في `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة من الأدلة (بداية مارس 2025)

أحدث المصنوعات اليدوية/الإنتاجية المرجعية في خريطة الطريق والدقائق
يعيش في دلو `s3://sora-governance/sorafs/gateway_dns/`. لوس تجزئات أباجو
مراجعة البيان الكنسي (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **التشغيل التجريبي — 02-03-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - حزمة Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - تفاصيل ملف PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة عمل حية — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(الشحن المعلق: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel يضيف SHA-256 عندما يتم عرض PDF بالكامل في الحزمة.)_

## مادة ذات صلة

- [دليل العمليات الخاص بالبوابة](./operations-playbook.md)
- [خطة المراقبة SoraFS](./observability-plan.md)
- [متعقب DNS اللامركزي والبوابة](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)