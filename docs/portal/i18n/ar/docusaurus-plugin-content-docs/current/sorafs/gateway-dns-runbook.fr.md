---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de lancement Gateway & DNS SoraFS

تعكس هذه النسخة من الباب دليل التشغيل Canonique dans
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
يمكنك التقاط عمليات المراقبة المستمرة لتدفق عمل DNS اللامركزي والبوابة
حتى يتمكن المسؤولون من الشبكات والعمليات والتوثيق من تكرار الكومة
الأتمتة الأمامية للرمح 2025-03.

## Portée et livrables

- قم بكتابة أسماء DNS (SF-4) والبوابة (SF-5) على أساس الاشتقاق المحدد
  des hôtes، وإصدارات ذخيرة المحللين، وأتمتة TLS/GAR، وما إلى ذلك
  التقاط الصور المسبقة.
- حفظ مدخلات البداية (جدول الأعمال، الدعوة، متابعة الحضور، لقطة
  Télémétrie GAR) تتم مزامنته مع أحدث تأثيرات المالك.
- إنتاج حزمة من العناصر القابلة للتدقيق لمراجعي الحوكمة: ملاحظات
  إصدار ذخيرة أدوات الحل، وسجلات بوابة المسبار، وفرز الحزام
  التوافق والتركيب Docs/DevRel.

## الأدوار والمسؤوليات| مسار العمل | المسؤوليات | التحف تتطلب |
|------------|------------------|------------------|
| الشبكات TL (كومة DNS) | الاستمرار في تحديد خطة الرحلات، وتنفيذ إصدارات مرجع RAD، ونشر مدخلات المحللين عن بعد. | `artifacts/soradns_directory/<ts>/`، يختلف عن `docs/source/soradns/deterministic_hosts.md`، وهو RAD الموضح. |
| قائد أتمتة العمليات (البوابة) | قم بتنفيذ تدريبات أتمتة TLS/ECH/GAR، وlancer `sorafs-gateway-probe`، وتشغيل خطافات PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`، مسبار JSON، الإدخالات `ops/drill-log.md`. |
| ضمان الجودة والأدوات WG | Lancer `ci/check_sorafs_gateway_conformance.sh`، معالج التركيبات، أرشفة الحزم الذاتية Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| مستندات / ديفريل | التقط الدقائق، واقرأ القراءة المسبقة للتصميم + الملحقات يوميًا، وانشر خلاصة الأدلة في هذه البوابة. | الملفات `docs/source/sorafs_gateway_dns_design_*.md` لليوم وملاحظات الطرح. |

## المدخلات والمتطلبات- مواصفات المحددات (`docs/source/soradns/deterministic_hosts.md`) وآخرون
  السقالات شهادة المحلل (`docs/source/soradns/resolver_attestation_directory.md`).
- بوابة المصنوعات اليدوية: المشغل اليدوي، مساعدو التشغيل الآلي TLS/ECH،
  التوجيه المباشر، سير العمل، الشهادة الذاتية `docs/source/sorafs_gateway_*`.
- الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh`، والمساعدين CI
  (`ci/check_sorafs_gateway_conformance.sh`، `ci/check_sorafs_gateway_probe.sh`).
- الأسرار: مفتاح إصدار GAR، وبيانات الاعتماد ACME DNS/TLS، ومفتاح التوجيه PagerDuty،
  رمز المصادقة Torii لجلب المحللات.

## قائمة المراجعة للمجلد المسبق

1. قم بتأكيد المشاركين وجدول الأعمال حتى الآن
   `docs/source/sorafs_gateway_dns_design_attendance.md` ونشر جدول الأعمال
   كورانت (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. قم بإعداد جذور القطع الأثرية التي تخبرك بذلك
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` وآخرون
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. قم بتفكيك التركيبات (بيانات GAR، Preuves RAD، حزم بوابة المطابقة) وما إلى ذلك
   تأكد من أن الحالة `git submodule` تتوافق مع أحدث علامة تدريب.
4. التحقق من الأسرار (مفتاح الإصدار Ed25519، ملف حساب ACME، الرمز المميز PagerDuty)
   والمراسلات الخاصة بك مع المجموع الاختباري للقبو.
5. قم بإجراء اختبار دخان لأسلاك القياس عن بعد (نقطة النهاية Pushgateway، اللوحة GAR Grafana)
   قبل الحفر.

## حلقات بروفة الأتمتة

### Carte d'hôtes Déterministe & Release du répertoire RAD1. تنفيذ المساعدة في تحديد الأسعار على مجموعة البيانات
   اقتراح وتأكيد عدم وجود الانجراف على أساس العلاقة
   `docs/source/soradns/deterministic_hosts.md`.
2. إنشاء حزمة من ذخيرة الحلول :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. قم بتسجيل معرف المرجع وSHA-256 وخطوط الطلعة المطبوعة في
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` وفي دقائق انطلاق المباراة.

### التقاط DNS عن بعد

- قم بتخصيص سجلات الشفافية لمدة تزيد عن 10 دقائق
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير عدادات Pushgateway وأرشفة اللقطات NDJSON إلى جانبك
  مرجع معرف التشغيل.

### تدريبات على أتمتة البوابة

1. قم بتشغيل مستشعر TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. لانسر لو تسخير المطابقة (`ci/check_sorafs_gateway_conformance.sh`) وآخرون
   الشهادة الذاتية المساعدة (`scripts/sorafs_gateway_self_cert.sh`) لتفكيك الحزمة
   الشهادات Norito.
3. التقط أحداث PagerDuty/Webhook للتحقق من سلسلة الأتمتة
   fonctionne de bout en bot.

### التعبئة والتغليف Preuves

- تحديث `ops/drill-log.md` مع الطوابع الزمنية والمشاركين وأجزاء التحقيق.
- تخزين القطع الأثرية في سجلات تشغيل ID ونشر توليف تنفيذي
  dans les Minutes Docs/DevRel.
- ضع حزمة الإجراءات في تذكرة الإدارة قبل إعادة انطلاق المباراة.

## الرسوم المتحركة للجلسة وخلاصة العروض المسبقة- **الجدول الزمني للمدير :**
  - T-24 ساعة — إدارة البرامج بعد الهبوط + لقطة جدول الأعمال/التواجد في `#nexus-steering`.
  - T-2 h — تقوم شبكة TL بتصوير لقطة عن بعد لـ GAR وإرسال الدلتا إلى `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — تقوم Ops Automation بالتحقق من تحضير المجسات وكتابة معرف التشغيل النشط في `artifacts/sorafs_gateway_dns/current`.
  - قلادة الاستئناف — Le modérateur Partage ce runbook et تعين كاتبًا بشكل مباشر ; يلتقط Docs/DevRel الإجراءات عبر الإنترنت.
- **قالب الدقائق:** انسخ القالب
  `docs/source/sorafs_gateway_dns_design_minutes.md` (أيضًا مرآة في الحزمة
  du portail) والتزم بمثيل يعوض الجلسة. قم بتضمين القائمة
  المشاركون، والقرارات، والإجراءات، وعلامات التقدم والمخاطر المفتوحة.
- **Upload des preuves :** Zipper le répertoire `runbook_bundle/` du rehearsal,
  انضم إلى ملف PDF الخاص بالدقائق، وقم بتسجيل التجزئات SHA-256 في الدقائق +
  جدول الأعمال، ثم اضغط على الاسم المستعار لمراجعي الحوكمة مرة واحدة في التحميلات
  متوفر في `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة من Preuves (بداية المريخ 2025)

آخر الأعمال الفنية بروفة/مرجع مباشر في خريطة الطريق والدقائق
يوجد مخزون في الجرافة `s3://sora-governance/sorafs/gateway_dns/`. التجزئة
يعكس هذا البيان الكنسي (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **التشغيل التجريبي — 02-03-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - حزمة القطران: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - دقائق PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **البث المباشر — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(تحميل بحذر: `gateway_dns_minutes_20250303.pdf` — يضيف Docs/DevRel SHA-256 لأن PDF سيظهر داخل الحزمة.)_

##العتاد connexe

- [بوابة قواعد اللعبة العملية](./operations-playbook.md)
- [خطة إمكانية المراقبة SoraFS](./observability-plan.md)
- [بوابة وبوابة تعقب DNS اللامركزية](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)