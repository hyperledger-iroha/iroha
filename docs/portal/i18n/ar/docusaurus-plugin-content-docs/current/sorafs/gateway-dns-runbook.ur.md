---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS الشبكة وDNS

إنها لعبة كمبيوتر محمول تمنع منع البطاقات تمامًا
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) .
إنها شبكة DNS وبوابة لا مركزية تعمل على شبكة الإنترنت الآمنة
نيويورك، آبس، ونهاية 2025-03 هي أحدث صيحات الموضة
مشقي كر سكي..

## اسكوپ و ڈليوريبلز

- معالم DNS (SF-4) والبوابة (SF-5) التي يتم تحديدها، وهي اشتقاق المضيف الحتمي،
  إصدارات دليل المحلل، وأتمتة TLS/GAR، والتقاط الأدلة هي أمور معقدة تشمل الجميع.
- کک اف ان پٹس (جدول الأعمال، دعوة، تعقب الحضور، لقطة القياس عن بعد GAR) کو
  لقد أصبحت الآن أحدث تعيينات المالك.
- ريوورز تحتوي على حزمة أعمال فنية قابلة للتنفيذ: دليل المحلل
  ملاحظات الإصدار، وسجلات مسبار البوابة، ومخرجات مجموعة أدوات التوافق، وملخص المستندات/DevRel۔

## رولز وذمہ دارياں| العمل | ذہ دارياں | مطلوبہ تحف |
|------------|-----------|------------------|
| شبكات TL (مكدس DNS) | خطة المضيف الحتمية برقرار رکھنا، إصدارات دليل RAD چلانا، مدخلات القياس عن بعد للمحلل شاع کرنا۔ | `artifacts/soradns_directory/<ts>/`، `docs/source/soradns/deterministic_hosts.md` هو الفرق، وبيانات تعريف RAD. |
| قائد أتمتة العمليات (البوابة) | تدريبات التشغيل الآلي TLS/ECH/GAR چلانا، `sorafs-gateway-probe` چلانا، خطافات PagerDuty پڈیٹ کرنا. | `artifacts/sorafs_gateway_probe/<ts>/`، مسبار JSON، إدخالات `ops/drill-log.md`۔ |
| ضمان الجودة والأدوات WG | `ci/check_sorafs_gateway_conformance.sh` Đlana، التركيبات کیوریٹ كرنا، Norito حزم الشهادة الذاتية آرکائیو کرنا. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| مستندات / ديفريل | دقيقة من سجل كرنا، تصميم قراءة مسبقة + ملاحق اپڈیٹ کرنا، وملخص الأدلة الشائع کرنا. | تمت الإشارة إليه مع `docs/source/sorafs_gateway_dns_design_*.md` الفائض وملاحظات الطرح. |

## ان پٹس و پري ريكويرمنٹس

- مواصفات المضيف الحتمية (`docs/source/soradns/deterministic_hosts.md`) والمحلل
  سقالات التصديق (`docs/source/soradns/resolver_attestation_directory.md`).
- مصنوعات البوابة: دليل المشغل، مساعدي أتمتة TLS/ECH، توجيه الوضع المباشر،
  وسير عمل الشهادة الذاتية هو `docs/source/sorafs_gateway_*` تحت ہے.
- الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh`، ومساعدي CI
  (`ci/check_sorafs_gateway_conformance.sh`، `ci/check_sorafs_gateway_probe.sh`).
- الأسرار: مفتاح إصدار GAR، بيانات اعتماد DNS/TLS ACME، مفتاح توجيه PagerDuty،
  ويقوم المحلل بجلب رمز المصادقة Torii.

## پری فلائٹ چیک لٹ1. `docs/source/sorafs_gateway_dns_design_attendance.md` الشركة وجدول الأعمال
   قم بقراءة جدول الأعمال وجدول الأعمال الموجود (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2.`artifacts/sorafs_gateway_dns/<YYYYMMDD>/` و
   `artifacts/soradns_directory/<YYYYMMDD>/` جذور المصنوعات اليدوية.
3. التركيبات (بيانات GAR، إثباتات RAD، حزم توافق البوابة)
   هذه هي المرة الأولى التي يتم فيها استخدام علامة `git submodule` كعلامة بروفة.
4. الأسرار (مفتاح الإصدار Ed25519، ملف حساب ACME، رمز PagerDuty)
   المجاميع الاختبارية للقبو هي نفس الشيء.
5. حفر أهداف القياس عن بعد (نقطة نهاية بوابة الدفع، لوحة GAR Grafana) لاختبار الدخان.

## آٹومیشن خطوات البروفة

### خريطة المضيف الحتمية وإصدار دليل RAD

1. يظهر البحث عن بعض الأخطاء في مساعد اشتقاق المضيف الحتمي
   لا يوجد تصديق على `docs/source/soradns/deterministic_hosts.md` مقابل الانجراف.
2. سلسلة دليل المحلل:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. معرف الدليل الأصلي، SHA-256، ومسارات الإخراج
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ودقائق البداية تساعد في تسجيل الأهداف.

### التقاط القياس عن بعد DNS

- سجلات شفافية المحلل التي تزيد عن ≥10 دقائق:
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تقوم مقاييس Pushgateway بتصدير لقطات CRIC و NDJSON لتشغيل دليل المعرفات، مما يؤدي إلى إنشاء سجل إحصائي.

### تدريبات أتمتة البوابة

1. مسبار TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```2. حزام المطابقة (`ci/check_sorafs_gateway_conformance.sh`) ومساعد الشهادة الذاتية
   (`scripts/sorafs_gateway_self_cert.sh`) قم بتجديد حزمة الشهادات Norito.
3. تقوم أحداث PagerDuty/Webhook بالتقاط خاصية الحركة التلقائية من طرف إلى طرف بشكل ثابت.

### تغليف الأدلة

- `ops/drill-log.md` يحتوي على الطوابع الزمنية والمشاركين وتجزئات التحقيق.
- تشغيل أدلة المعرفات التي تتضمن عناصر محفوظات کریں ودقائق Docs/DevRel عبارة عن ملخص تنفيذي شاع کریں.
- مراجعة انطلاقة سلسلة من تذكرة الحوكمة تحتوي على حزمة الأدلة لنك كري.

## سیشن فیسلیٹیشن وتسليم الأدلة- **الجدول الزمني للمشرف:**
  - T-24 h — إدارة البرنامج `#nexus-steering` تذكير + لقطة جدول الأعمال/الحضور پوسٹ کرے۔
  - T-2 h — لقطة القياس عن بعد لشبكة TL GAR محدثة `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` من دلتا التسجيل.
  - T-15 m — جاهزية مسبار أتمتة العمليات ومعرف التشغيل النشط `artifacts/sorafs_gateway_dns/current` مفعل.
  - كل کے دوران — المشرف هو أكثر كرے وتعيين الكاتب المباشر کرے؛ يتم تسجيل عناصر العمل المضمنة في Docs/DevRel.
- **نموذج الدقيقة:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` عبارة عن هيكل عظمي للكريات (تتوفر حزمة البوابة أيضًا) ويتم الالتزام بشبكة لمثيل كامل. قائمة الحضور، والقرارات، وعناصر العمل، وتجزئات الأدلة، والمخاطر القائمة
- **تحميل الأدلة:** دليل بروفة `runbook_bundle/` ملف مضغوط، محضر تقديم PDF إرفاق ملف، دقائق + جدول أعمال SHA-256 تجزئات، والتحميلات بعد الاسم المستعار لمراجع الإدارة ping زائر `s3://sora-governance/sorafs/gateway_dns/<date>/` في بانشا جاي.

## لقطة الأدلة (بداية مارس 2025)

خريطة الطريق والدقائق التي تم إجراؤها هي أفضل بروفة/مصنوعات حية
`s3://sora-governance/sorafs/gateway_dns/` دلو. نیچے دیے گئے التجزئات
البيان القانوني (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) يعكس القانون.- **التشغيل التجريبي — 02-03-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - حزمة القطران: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - المحضر PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة عمل مباشرة — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(في انتظار التحميل: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF آنے پر SHA-256 شامل کرے گا.)_

## مواد ذات صلة

- [دليل تشغيل عمليات البوابة](./operations-playbook.md)
- [خطة إمكانية المراقبة SoraFS](./observability-plan.md)
- [متعقب DNS والبوابة اللامركزي](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)