---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3dda37ec8b2effd240452f780368ebdfaa813d84daa5b46bfbfea587f6715fb
source_last_modified: "2025-11-14T04:43:21.735486+00:00"
translation_last_reviewed: 2026-01-30
---

# دليل تشغيل انطلاقة Gateway وDNS في SoraFS

تعكس نسخة البوابة هذا الدليل التشغيلي المعتمد الموجود في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
وهي تلتقط الضوابط التشغيلية لمسار Decentralized DNS & Gateway كي تتمكن فرق
الشبكات والعمليات والتوثيق من تدريب حزمة الأتمتة قبل انطلاقة 2025-03.

## النطاق والمخرجات

- ربط معالم DNS (SF-4) وgateway (SF-5) عبر تمارين اشتقاق المضيفات الحتمي،
  وإصدارات دليل resolvers، وأتمتة TLS/GAR، وجمع الأدلة.
- إبقاء مدخلات الانطلاقة (الأجندة، الدعوة، متعقب الحضور، لقطة تليمترية GAR)
  متزامنة مع آخر تعيينات المالكين.
- إنتاج حزمة آرتيفاكتات قابلة للتدقيق لمراجعي الحوكمة: ملاحظات إصدار دليل
  resolvers، سجلات فحوصات gateway، مخرجات أداة التوافق، وملخص Docs/DevRel.

## الأدوار والمسؤوليات

| المسار | المسؤوليات | الآرتيفاكتات المطلوبة |
|--------|-------------|------------------------|
| Networking TL (حزمة DNS) | الحفاظ على خطة المضيفات الحتمية، تشغيل إصدارات دليل RAD، نشر مدخلات تليمترية resolvers. | `artifacts/soradns_directory/<ts>/`، فروق `docs/source/soradns/deterministic_hosts.md`، وبيانات RAD الوصفية. |
| Ops Automation Lead (gateway) | تنفيذ تمارين أتمتة TLS/ECH/GAR، تشغيل `sorafs-gateway-probe`، وتحديث خطافات PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`، JSON للـ probe، ومدخلات `ops/drill-log.md`. |
| QA Guild & Tooling WG | تشغيل `ci/check_sorafs_gateway_conformance.sh`، تنسيق fixtures، وأرشفة حزم self-cert الخاصة بـ Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`، `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | تدوين المحاضر، تحديث pre-read التصميم + الملاحق، ونشر ملخص الأدلة في هذا البوابة. | ملفات `docs/source/sorafs_gateway_dns_design_*.md` المحدثة وملاحظات الإطلاق. |

## المدخلات والمتطلبات المسبقة

- مواصفة المضيفات الحتمية (`docs/source/soradns/deterministic_hosts.md`) وبنية
  اعتماد resolvers (`docs/source/soradns/resolver_attestation_directory.md`).
- آرتيفاكتات gateway: دليل المشغل، مساعدات أتمتة TLS/ECH، إرشادات direct-mode،
  ومسار self-cert ضمن `docs/source/sorafs_gateway_*`.
- الأدوات: `cargo xtask soradns-directory-release`،
  `cargo xtask sorafs-gateway-probe`، `scripts/telemetry/run_soradns_transparency_tail.sh`،
  `scripts/sorafs_gateway_self_cert.sh`، وأدوات CI المساعدة
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- الأسرار: مفتاح إصدار GAR، بيانات اعتماد ACME لـ DNS/TLS، مفتاح توجيه PagerDuty،
  ورمز مصادقة Torii لجلب resolvers.

## قائمة التحقق قبل التنفيذ

1. تأكيد الحضور والأجندة بتحديث
   `docs/source/sorafs_gateway_dns_design_attendance.md` وتعميم الأجندة الحالية
   (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. تجهيز جذور الآرتيفاكتات مثل
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` و
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. تحديث fixtures (manifests الخاصة بـ GAR، أدلة RAD، حزم توافق gateway) مع التأكد
   من أن حالة `git submodule` تطابق آخر وسم تدريب.
4. التحقق من الأسرار (مفتاح إصدار Ed25519، ملف حساب ACME، رمز PagerDuty) ومطابقة
   checksums في vault.
5. تنفيذ smoke-test لأهداف التليمترية (endpoint الخاص بـ Pushgateway، لوحة GAR في Grafana)
   قبل التمرين.

## خطوات تمرين الأتمتة

### خريطة المضيفات الحتمية وإصدار دليل RAD

1. تشغيل مساعد اشتقاق المضيفات الحتمي على مجموعة manifests المقترحة والتأكد
   من عدم وجود drift مقارنةً بـ
   `docs/source/soradns/deterministic_hosts.md`.
2. إنشاء حزمة دليل resolvers:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. تدوين معرّف الدليل المطبوع وSHA-256 ومسارات الإخراج داخل
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` وفي محاضر الانطلاقة.

### التقاط تليمترية DNS

- تتبع سجلات شفافية resolvers لمدة ≥10 دقائق عبر
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير مقاييس Pushgateway وأرشفة لقطات NDJSON بجانب مجلد run ID.

### تمارين أتمتة gateway

1. تشغيل فحص TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. تشغيل أداة التوافق (`ci/check_sorafs_gateway_conformance.sh`) ومساعد self-cert
   (`scripts/sorafs_gateway_self_cert.sh`) لتحديث حزمة اعتماد Norito.
3. التقاط أحداث PagerDuty/Webhook لإثبات أن مسار الأتمتة يعمل end-to-end.

### تجميع الأدلة

- تحديث `ops/drill-log.md` بالطوابع الزمنية والمشاركين وهاشات probe.
- تخزين الآرتيفاكتات ضمن مجلدات run ID ونشر ملخص تنفيذي ضمن محاضر Docs/DevRel.
- ربط حزمة الأدلة في تذكرة الحوكمة قبل مراجعة الانطلاقة.

## إدارة الجلسة وتسليم الأدلة

- **الخط الزمني للمشرف:**
  - T-24 h — ينشر Program Management التذكير + لقطة الأجندة/الحضور في `#nexus-steering`.
  - T-2 h — يقوم Networking TL بتحديث لقطة تليمترية GAR وتسجيل الفروقات في `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — يتحقق Ops Automation من جاهزية probes ويكتب run ID النشط في `artifacts/sorafs_gateway_dns/current`.
  - أثناء المكالمة — يشارك المشرف هذا الدليل ويُعيّن ناسخًا مباشرًا؛ وتلتقط Docs/DevRel بنود العمل أثناء الجلسة.
- **قالب المحاضر:** انسخ الهيكل من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (ومنعكس في bundle البوابة)
  والتزم بإيداع نسخة مكتملة لكل جلسة. تضمّن قائمة الحضور والقرارات وبنود العمل
  وهاشات الأدلة والمخاطر المفتوحة.
- **رفع الأدلة:** اضغط مجلد `runbook_bundle/` الخاص بالتمرين، أرفق PDF المحاضر
  المُصدّر، وسجّل هاشات SHA-256 في المحاضر + الأجندة، ثم نبّه اسم المراجعين
  المعتمدين بعد رفع الملفات إلى `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة الأدلة (انطلاقة مارس 2025)

آخر الآرتيفاكتات المرتبطة بالخارطة والمحاضر محفوظة في
`s3://sora-governance/sorafs/gateway_dns/`. الهاشات أدناه تعكس
الـ manifest المعتمد (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball الحزمة: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF المحاضر: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة مباشرة — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(رفع متوقع: `gateway_dns_minutes_20250303.pdf` — ستضيف Docs/DevRel قيمة SHA-256 عند توفر PDF في الحزمة.)_

## مواد ذات صلة

- [دليل تشغيل عمليات gateway](./operations-playbook.md)
- [خطة مراقبة SoraFS](./observability-plan.md)
- [متعقّب DNS اللامركزي وgateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
