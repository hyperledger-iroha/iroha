---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تشغيل Gateway وDNS في SoraFS

قراءة الشعار لهذا الدليل العامل المعتمد الموجود في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
وهي تتأهل للعملية لمسار Decentralized DNS & Gateway كي للبدء بفرق
شبكة الاتصالات القضائية والتوثيق من تدريب القدرات قبل القفزة 2025-03.

## النطاق والمخرجات

- ربط معالم DNS (SF-4) والبوابة (SF-5) عبر تمارين اشتقاق الأطباق الحتمي،
  وإصدارات دليل الحلول، وطباعة TLS/GAR، تحكم الأدلة.
- تتضمن مدخلات الانطلاقة (الأجندة، الأحداث، متعقب الحضور، لقطة تليمترية GAR)
  متزامنة مع آخر تعيينات الملاكين.
- إنتاج حزمة آرتيفاكتات قابلة للتدقيق لمراجعي التصفح: دليل إصدار التعليقات
  أدوات الحل، سجلات فحوصات البوابة، مخرجات أداة التوافق، وملخص Docs/DevRel.

## الضبط والمسؤوليات| المسار | المسؤوليات | الآرتيفاكتات المطلوبة |
|--------|------------|------------------------|
| شبكات TL (حزمة DNS) | تقدر على تشغيل خطة التشغيل الخاصة بالحتمية، إصدار دليل RAD، نشر المدخلات الليمترية للحلول. | `artifacts/soradns_directory/<ts>/`، فروق `docs/source/soradns/deterministic_hosts.md`، وبيانات RAD الوصفية. |
| قائد أتمتة العمليات (البوابة) | تنفيذ عمليات تصميم TLS/ECH/GAR، تشغيل `sorafs-gateway-probe`، وتحديث خطافات PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`، JSON للـ مسبار، ومدخلات `ops/drill-log.md`. |
| ضمان الجودة والأدوات WG | تشغيل `ci/check_sorafs_gateway_conformance.sh`، التركيبات القديمة، تحاول اكتشاف حزم الشهادات الذاتية الخاصة بـ Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| مستندات / ديفريل | تدوين المحاضر، تحديث التحرير المسبق + الملاحق، نشر ملخص الأدلة في هذه البوابة. | ملفات `docs/source/sorafs_gateway_dns_design_*.md` المحدثة والملاحظات الفارقة. |

## المدخلات والمتطلبات المسبقة

- وصفة محلية الحتمية (`docs/source/soradns/deterministic_hosts.md`) وبنية
  وحدات الحل المعتمدة (`docs/source/soradns/resolver_attestation_directory.md`).
- آرتيفاكتات البوابة: دليل التحرير، مساعدات محاسبة TLS/ECH، إرشادات الوضع المباشر،
  ومسار الشهادة الذاتية ضمن `docs/source/sorafs_gateway_*`.
-الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh`، الأداة المساعدة CI
  (`ci/check_sorafs_gateway_conformance.sh`، `ci/check_sorafs_gateway_probe.sh`).
- الاستخدام: مفتاح الإصدار GAR، بيانات معتمدة من ACME لـ DNS/TLS، مفتاح توجيه PagerDuty،
  ورمز مصادقة Torii لجلب المحللين.

## قائمة التحقق قبل التنفيذ1. تأكيد الحضور والأجندة
   `docs/source/sorafs_gateway_dns_design_attendance.md` وتميم الأجندة الحالية
   (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. تجهيزات ضرورية مثل
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` و
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. تحديث التركيبات (البيانات الخاصة بـ GAR، إثبات RAD، حزم توافق البوابة) مع كاتب
   من أن حالة `git submodule` تطابق آخر موعد للزواج.
4. التحقق من المستخدم (مفتاح الإصدار Ed25519، ملف حساب ACME، رمز PagerDuty) والمطابقة
   المجاميع الاختبارية في قبو.
5. تنفيذ اختبار الدخان للأهداف التليميترية (endpoint الخاص بـ Pushgateway، لوحة GAR في Grafana)
   قبل ممارسة.

##تدرب السعة

### خريطة المضيفات الحتمية وإصدار دليل RAD

1. تشغيل مساعد اشتقاق المطعمات هتمي على مجموعة البيانات المسبقة والتجهيزية
   من عدم وجود الانجراف بـ
   `docs/source/soradns/deterministic_hosts.md`.
2. إنشاء حزمة أدوات حل الدليل:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. تدوين مُعرّف الدليل المطبوع وSHA-256 ومسارات محتوى داخل
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` وفي محاضرة الانطلاقة.

### التقاط تليمترية DNS

- تتبع المحلول الإحصائي لـ ≥10 للضغط عبر
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير معايير Pushgateway واكتشف لقطات NDJSON بدءًا من معرف تشغيل المجلد.

### تمارين بوابة

1. فحص تشغيل TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. تشغيل أداة التوافق (`ci/check_sorafs_gateway_conformance.sh`) والشهادة الذاتية المساعدة
   (`scripts/sorafs_gateway_self_cert.sh`) للتحديث حزمة موثوق Norito.
3. التقاط أحداث PagerDuty/Webhook لإثبات أن مسار المحاسبة يعمل من النهاية إلى النهاية.

### تجميع الأدلة- تحديث `ops/drill-log.md` بالطوابع الزمنية والمشاركين وهاشات التحقيق.
- تخزين الآرتيفاكتات ضمن مجلدات run ID ونشر ملخصي ضمن محاضر Docs/DevRel.
- ربط حزمة الأدلة في تذكرة التفاصيل قبل مراجعة الانطلاقة.

##إدارة الجلسة وتسلم الأدلة

- **الخطير للمشرف:**
  - T-24 h — ينشر إدارة البرامج التذكير + لقطة الأجندة/الحضور في `#nexus-steering`.
  - T-2 h — يقوم Networking TL لقطة تليمترية GAR وتسجيل الفروقات في `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — يحقق Ops Automation من مجسات جاهزة ويكتب تشغيل ID المفضل في `artifacts/sorafs_gateway_dns/current`.
  - أثناء عدم — يشارك المشرف على هذا الدليل ويُعين بصفة عامة مباشرًا؛ وقم باختيار Docs/DevRel بنود العمل أثناء الجلسة.
- **قالب المحاضر:** انسخ الهيكل من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (ومنعكس في حزمة البوابة)
  والتزم بإيداع نسخة مكتملة لكل جلسة. تذهل قائمة الحضور والقرارات وبنوود العمل
  وهاشات الأدلة والمخاطر المفتوحة.
- **رفع الأدلة:** ضغط الأسطوانة `runbook_bundle/` الخاص بالتمرين، أرفق PDF المحاضر
  المُصدّر، وسجّل هاشات SHA-256 في المحاضر + الأجندة، ثم نبّه اسم المراجعين
  مؤهلين بعد رفع الملفات إلى `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة الأدلة (انطلاقة مارس 2025)

آخر الآرتيفاكتات بعد بالخارطة والمحاضر المحفوظة في
`s3://sora-governance/sorafs/gateway_dns/`. الهاتشات أدناه
الـ مانفيست مؤهل (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **التشغيل التجريبي — 02-03-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - مجموعة القطران: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF المحاضر: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة مباشرة — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(رفع المتوقع: `gateway_dns_minutes_20250303.pdf` — للاطلاع على قيمة Docs/DevRel SHA-256 عند توفر PDF في الحزمة.)_

## مادة ذات صلة

- [دليل تشغيل عمليات البوابة](./operations-playbook.md)
- [خطة مراقبة SoraFS](./observability-plan.md)
- [متقن DNS اللامركزي والبوابة](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)