---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بوابة رانبوك و DNS SoraFS

هذه النسخة الموجودة في البوابة توزع رصيدًا قانونيًا في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
هذه هي حواجز الحماية التشغيلية المثالية لـ DNS والبوابة اللامركزية،
يمكن لإرشادات الشبكات والعمليات والوثائق أن تتكرر
الأتمتة قبل انطلاق 2025-03.

## التسليم والتسليمات

- قم بتوصيل كل DNS (SF-4) والبوابة (SF-5) من خلال التحديدات المتكررة
  مشتقات المشتقات، ومحللات كتالوج النتائج، وأتمتة TLS/GAR،
  сбора доказательств.
- متابعة التحف الفنية (جدول الأعمال، الدعوة، تعقب الحضور، لقطة
  أجهزة القياس عن بعد GAR) متزامنة مع أصحاب الأسماء الأخيرة.
- إضافة عناصر الحزمة الصوتية لمراجعي الحوكمة: ملاحظات الإصدار
  محللات الكتالوجات، مجسات بوابة السجل، أحزمة مطابقة المياه والإمدادات
  المستندات/DevRel.

## رولي والصراحة| مسار العمل | ردود الفعل | التحف الفنية |
|------------|-----------------|-----|
| شبكات TL (مكدس DNS) | دعم تحديد خطط المضيفين، وبدء إصدارات دليل RAD، ونشر محللات القياس عن بعد. | `artifacts/soradns_directory/<ts>/`، يختلف عن `docs/source/soradns/deterministic_hosts.md`، بيانات تعريف RAD. |
| قائد أتمتة العمليات (البوابة) | استخدم أتمتة التدريبات TLS/ECH/GAR، وأغلق `sorafs-gateway-probe`، وأحدث الخطافات PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`، مسبار JSON، مكتوب في `ops/drill-log.md`. |
| ضمان الجودة والأدوات WG | قم بتسجيل `ci/check_sorafs_gateway_conformance.sh`، وشراء التركيبات، وأرشفة حزم الشهادة الذاتية Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| مستندات / ديفريل | иксировать دقائق، تصميم قراءة مسبقة + الملاحق، نشر ملخص الأدلة في البوابة. | Обновленные `docs/source/sorafs_gateway_dns_design_*.md` وملاحظات الطرح. |

## المتطلبات والمتطلبات الأساسية

- مواصفات تحديد المضيفين (`docs/source/soradns/deterministic_hosts.md`) و
  شهادة لمحللي (`docs/source/soradns/resolver_attestation_directory.md`).
- بوابة المصنوعات اليدوية: دليل المشغل، مساعدي أتمتة TLS/ECH،
  إرشادات حول الوضع المباشر وسير عمل الشهادة الذاتية ضمن `docs/source/sorafs_gateway_*`.
- الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh` ومساعدي CI
  (`ci/check_sorafs_gateway_conformance.sh`، `ci/check_sorafs_gateway_probe.sh`).
- الأسرار: مفتاح إصدار GAR وبيانات اعتماد DNS/TLS ACME ومفتاح التوجيه PagerDuty،
  Torii رمز المصادقة لجلب أدوات الحل.

## قائمة التحقق قبل الرحلة1. التحقق من المديرين وجدول الأعمال، ملاحظة
   `docs/source/sorafs_gateway_dns_design_attendance.md` و رازوسلاف تيكويوس
   جدول الأعمال (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. قم بإدراج القطع الأثرية الصغيرة، على سبيل المثال
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` و
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. شراء التركيبات (بيانات GAR، وإثباتات RAD، وبوابة توافق الحزم) وغيرها
   تأكد من أن الحالة `git submodule` تستجيب لعلامة التدريب التالية.
4. التحقق من الأسرار (مفتاح إصدار Ed25519، وملف حساب ACME، ورمز PagerDuty) و
   المجاميع الاختبارية المرغوبة في القبو.
5. التحقق من أهداف القياس عن بعد لاختبار الدخان (نقطة نهاية Pushgateway، لوحة GAR Grafana)
   قبل الحفر.

## أتمتة التكرارات

### تحديد بطاقة المضيفين وإصدار كتالوج RAD

1. قم بتثبيت المساعد في تحديد مجموعة المشتقات المقترحة
   البيانات والتحقق من الانجراف بشكل متكرر
   `docs/source/soradns/deterministic_hosts.md`.
2. تصميم وحدات حل كتالوج الحزمة:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. قم بتأكيد كتالوج المعرفات النهائية وSHA-256 وإدخالات الإدخال النهائية
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` و в دقائق البداية.

### تخصيص DNS عن بعد

- سجلات شفافية محلل الذيل في غضون ≥10 دقائق
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير مقاييس Pushgateway وأرشفة لقطات NDJSON معها
  معرف تشغيل الدليل.

### بوابة الأتمتة للتدريبات

1. استخدم مسبار TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```2. قم بتركيب حزام المطابقة (`ci/check_sorafs_gateway_conformance.sh`) و
   مساعد الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) للترقية
   حزمة التصديق Norito.
3. قم بتفعيل خدمة PagerDuty/Webhook لضمان العمل الشامل
   أتمتة الأتمتة.

### التعبئة والتغليف مضمونة

- قم بتحديث `ops/drill-log.md` باستخدام الطوابع الزمنية والعلامات والتجزئات.
- العناصر المصاحبة في الأدلة، قم بتشغيل معرف ونشر الملخص التنفيذي
  в دقائق Docs/DevRel.
- الحصول على حزمة الأدلة في تذكرة الحوكمة لمراجعة البداية.

## اعتدال الجلسة والتأكيد عليها- ** مشرف الجدول الزمني: **
  - T-24 h — التسمية العامة لإدارة البرنامج + لقطة جدول الأعمال/الحضور في `#nexus-steering`.
  - T-2 h — تقوم TL الخاصة بالشبكات بإظهار لقطة عن بعد لـ GAR وإصلاح الدلتا في `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m - تقوم Ops Automation بالتحقق من المجسات الفعلية وتسجيل معرف التشغيل النشط في `artifacts/sorafs_gateway_dns/current`.
  - في الوقت المحدد — يقوم المشرف بحذف هذا المقال ويسمي الكاتب المباشر؛ يقوم Docs/DevRel بإصلاح عناصر العمل في هذا الوقت.
- **دقائق شابلون:** انسخ الجدول من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (تم نقله أيضًا إلى حزمة البوابة)
  وقم بتخصيص نموذج متحرك لكل دورة. تشمل قائمة المشروبات،
  الحلول وعناصر العمل وأدلة التجزئة والمخاطر المكشوفة.
- **التعليق على الرسالة:** سجل `runbook_bundle/` من التدريب،
  استخدم دقائق PDF الشائعة، ولاحظ تجزئة SHA-256 في دقائق + جدول الأعمال،
  атем уведомите الاسم المستعار لمراجع الحوكمة بعد الدخول في
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة شاشة (بداية مارس 2025)

البروفة اللاحقة/العناصر الحية، التي يتم عرضها في خريطة الطريق والدقائق، يتم نقلها إلى دلو
`s3://sora-governance/sorafs/gateway_dns/`. هذا الأخير ينشر الكنسي
البيان (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **التشغيل التجريبي — 02-03-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - حزمة Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - المحضر PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة عمل مباشرة — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(في انتظار التحميل: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавит SHA-256 после попадания PDF в package.)_

## مواد صديقة للبيئة

- [دليل العمليات لبوابة العمليات](./operations-playbook.md)
- [خطة المراقبة SoraFS](./observability-plan.md)
- [Treker لامركزي DNS والبوابة] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)