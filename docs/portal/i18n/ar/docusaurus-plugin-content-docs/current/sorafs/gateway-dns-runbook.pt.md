---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تشغيل البوابة و DNS من SoraFS

تم نسخ هذه النسخة من البوابة الإلكترونية أو دليل التشغيل Canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
قم بالتقاط عمليات حواجز الحماية لتدفق عمل DNS اللامركزي والبوابة
لكي تتمكن من قراءة الشبكات والعمليات والتوثيق من البدء في الإزالة
الأتمتة المسبقة لبدء المباراة 2025-03.

## إسكوبو إي إنتريغافييس

- قم بتوصيل مواقع DNS (SF-4) والبوابة (SF-5) للاشتقاق
  تحديدية المضيفين، وإصدارات دليل الحلول، وأتمتة TLS/GAR
  والتقاط الأدلة.
- Manter os insumos do start (جدول الأعمال، دعوة، متتبع العرض، لقطة من
  القياس عن بعد GAR) متزامن مع أحدث خصائص المالكين.
- إنتاج حزمة من التحف التدقيقية لمراجعة الحوكمة: ملاحظات
  قم بتحرير دليل الحلول، وسجلات التحقيق في البوابة، وأداة تسخير دي
  التوافق وملخص Docs/DevRel.

## الباب والمسؤوليات| مسار العمل | المسؤوليات | Artefatos requeridos |
|------------|-------------------|------|
| شبكات TL (مكدس DNS) | بالإضافة إلى خطة تحديد المضيفين، يتم تنفيذ إصدارات دليل RAD ونشر مدخلات القياس عن بعد من المحللين. | `artifacts/soradns_directory/<ts>/`، يختلف عن `docs/source/soradns/deterministic_hosts.md`، البيانات التعريفية RAD. |
| قائد أتمتة العمليات (البوابة) | تنفيذ التدريبات التلقائية TLS/ECH/GAR، والقضيب `sorafs-gateway-probe`، وتحديث خطافات PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`، مسبار JSON، المدخلات `ops/drill-log.md`. |
| ضمان الجودة والأدوات WG | Rodar `ci/check_sorafs_gateway_conformance.sh`، تركيبات متجددة، استرجاع حزم الشهادة الذاتية Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| مستندات / ديفريل | قم بالتقاط الدقائق وتحديث القراءة المسبقة للتصميم + الملاحق ونشر ملخص الأدلة في هذه البوابة. | تم تحديث ملف Arquivos `docs/source/sorafs_gateway_dns_design_*.md` وملاحظات الطرح. |

## المدخلات والمتطلبات المسبقة- محددات المضيف المحددة (`docs/source/soradns/deterministic_hosts.md`) e
  o سقالات حل المشكلات (`docs/source/soradns/resolver_attestation_directory.md`).
- بوابة Artefatos: مشغل يدوي، مساعدين تلقائيين لـ TLS/ECH،
  إرشادات الوضع المباشر وسير العمل للشهادة الذاتية في `docs/source/sorafs_gateway_*`.
- الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh`، ومساعدي CI
  (`ci/check_sorafs_gateway_conformance.sh`، `ci/check_sorafs_gateway_probe.sh`).
- المفاتيح المنفصلة: مفتاح إصدار GAR واعتمادات ACME DNS/TLS ومفتاح التوجيه PagerDuty،
  رمز المصادقة Torii لجلب المحللين.

## قائمة المراجعة قبل الرحلة

1. قم بتأكيد المشاركين وتحديث جدول الأعمال
   `docs/source/sorafs_gateway_dns_design_attendance.md` ونشر جدول الأعمال
   حقيقي (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. قم بإعداد رايز دي أرتيفاتوس كومو
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` ه
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. تحديث التركيبات (بيانات GAR وإثباتات RAD وحزم البوابة المطابقة) e
   نضمن أن حالة `git submodule` ستستمر حتى النهاية.
4. التحقق من الانفصال (عنوان الإصدار Ed25519، ملف حساب ACME، رمز PagerDuty)
   e se bassem com المجموع الاختباري للقبو.
5. اختبار دخان الوجه لأهداف القياس عن بعد (نقطة النهاية Pushgateway، اللوحة GAR Grafana)
   الرهانات المسبقة تفعل الحفر.

## خطوات الإنشاء الآلي

### خريطة تحديدية للمضيفين وإصدار دليل RAD1. استعين بالمساعدة في الحصول على حتمية المضيفين مقابل مجموعة البيانات
   اقتراح وتأكيد أنه لا يوجد انجراف في العلاقة
   `docs/source/soradns/deterministic_hosts.md`.
2. قم بإعداد حزمة من أدوات الحل:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. قم بالتسجيل بمعرف المدير، أو SHA-256، وكاميرات الطباعة السعيدة داخل
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ولدينا دقائق من بداية المباراة.

### التقاط DNS للقياس عن بعد

- الحصول على سجلات شفافة من المحللين لمدة تزيد عن 10 دقائق باستخدام
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير مقاييس Pushgateway والوصول إلى لقطات نظام التشغيل NDJSON في نفس الوقت
  يقوم diretorio بتشغيل معرف.

### التدريبات الآلية على البوابة

1. قم بتنفيذ مسبار TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. ركب حزام الأمان (`ci/check_sorafs_gateway_conformance.sh`) ه
   o مساعد الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) للتحديث
   o حزمة البيانات Norito.
3. التقط أحداث PagerDuty/Webhook للتأكد من أتمتة الطريق
   funciona de ponta a ponta.

### تعبئة الأدلة

- تحديث `ops/drill-log.md` مع الطوابع الزمنية والمشاركين وتجزئة التحقيقات.
- إنشاء مجلدات معرف التشغيل ونشر السيرة الذاتية التنفيذية
  هذه دقائق قليلة من Docs/DevRel.
- قم بربط حزمة الأدلة في تذكرة الإدارة قبل المراجعة قبل انطلاق المباراة.

## تسهيل الجلسة وتسليم الأدلة- **ضبط وتيرة المشرف:**
  - T-24 h — بريد إدارة البرنامج + لقطة جدول الأعمال/العرض على `#nexus-steering`.
  - T-2 h — تحديث TL للشبكة أو لقطة القياس عن بعد GAR وتسجيل البيانات في `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — تقوم Ops Automation بالتحقق من طول المسبار وإخراج المعرف أو تشغيله في `artifacts/sorafs_gateway_dns/current`.
  - مدة المكالمة - المشرف يشارك هذا الدليل ويصممه للكتابة على قيد الحياة؛ يقوم Docs/DevRel بالتقاط العناصر المضمنة.
- **نموذج الدقائق:** انسخ هذا العدد
  `docs/source/sorafs_gateway_dns_design_minutes.md` (tambem espelhado بدون حزمة
  قم بالبوابة) وقم بإنشاء مثيل مسبق للجلسة. تشمل القائمة
  المشاركون، والقرارات، وأشياء من هذا القبيل، وتجزئات الأدلة والمخاطر المعلقة.
- **تحميل الأدلة:** الرمز البريدي للعنوان `runbook_bundle/` للتنزيل،
  قم بإرفاق ملف PDF بالدقائق المقدمة، وقم بتسجيل التجزئات SHA-256 في الدقائق +
  جدول الأعمال، وبعد تقديم المشورة أو الاسم المستعار لمراجعي الإدارة عند التحميلات
  chegarem em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة من الأدلة (بداية ماركو 2025)

آخر إبداعات البحث/المراجع المباشرة بدون خريطة طريق ودقائق قليلة
ficam لا دلو `s3://sora-governance/sorafs/gateway_dns/`. علامات التجزئة أبيكسو
espelham أو البيان الكنسي (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **التشغيل التجريبي — 02-03-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - حزمة Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF في الدقائق: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة عمل فيفو — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(تحميل معلق: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara o SHA-256 عندما يتم عرض PDF في الحزمة.)_

## مادة ذات صلة

- [دليل العمليات الخاص بالبوابة](./operations-playbook.md)
- [خطة المراقبة من SoraFS](./observability-plan.md)
- [بوابة تعقب DNS اللامركزية](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)