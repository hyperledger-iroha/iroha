---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تشغيل انطلاقة Gateway et DNS pour SoraFS

تعكس نسخة البوابة هذا الدليل التشغيلي المعتمد الموجود في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Plus d'informations sur DNS et passerelle décentralisés
Les événements et les événements se sont déroulés le 2025-03.

## النطاق والمخرجات

- Utilisez le DNS (SF-4) et la passerelle (SF-5) pour installer la passerelle DNS (SF-4) et la passerelle (SF-5).
  Il y a des résolveurs pour TLS/GAR et des résolveurs.
- إبقاء مدخلات الانطلاقة (الأجندة، الدعوة، متعقب الحضور، لقطة تليمترية GAR)
  متزامنة مع آخر تعيينات المالكين.
- إنتاج حزمة آرتيفاكتات قابلة للتدقيق لمراجعي الحوكمة: ملاحظات إصدار دليل
  résolveurs, est une passerelle, est une passerelle vers Docs/DevRel.

## الأدوار والمسؤوليات| المسار | المسؤوليات | الآرتيفاكتات المطلوبة |
|--------|-------------|--------------|
| Mise en réseau TL (DNS supplémentaire) | Il existe de nombreux résolveurs pour les résolveurs RAD. | `artifacts/soradns_directory/<ts>/`, et `docs/source/soradns/deterministic_hosts.md`, et RAD الوصفية. |
| Responsable de l'automatisation des opérations (passerelle) | Utilisez TLS/ECH/GAR pour `sorafs-gateway-probe` et PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON pour sonde, et `ops/drill-log.md`. |
| Guilde d'assurance qualité et groupe de travail sur les outils | Utilisez `ci/check_sorafs_gateway_conformance.sh`, les luminaires et l'auto-certification pour Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documents/DevRel | تدوين المحاضر، تحديث pre-read التصميم + الملاحق، ونشر ملخص الأدلة في هذا البوابة. | ملفات `docs/source/sorafs_gateway_dns_design_*.md` المحدثة وملاحظات الإطلاق. |

## المدخلات والمتطلبات المسبقة

- مواصفة المضيفات الحتمية (`docs/source/soradns/deterministic_hosts.md`) et
  Résolveurs اعتماد (`docs/source/soradns/resolver_attestation_directory.md`).
- Passerelle de passerelle : pour le mode direct TLS/ECH,
  L'auto-certification est `docs/source/sorafs_gateway_*`.
- Noms : `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, وأدوات CI المساعدة
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Fonction : GAR est connecté à ACME pour DNS/TLS et PagerDuty.
  Utilisez les résolveurs Torii.

## قائمة التحقق قبل التنفيذ1. تأكيد الحضور والأجندة بتحديث
   `docs/source/sorafs_gateway_dns_design_attendance.md` وتعميم الأجندة الحالية
   (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. تجهيز جذور الآرتيفاكتات مثل
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` et
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. تحديث luminaires (manifestes الخاصة بـ GAR، أدلة RAD، حزم توافق gateway) مع التأكد
   من أن حالة `git submodule` تطابق آخر وسم تدريب.
4. التحقق من الأسرار (مفتاح إصدار Ed25519, ملف حساب ACME, رمز PagerDuty) et مطابقة
   sommes de contrôle dans le coffre-fort.
5. Effectuer un test de fumée pour la fonction de point final (point final pour Pushgateway, pour GAR et Grafana)
   قبل التمرين.

## خطوات تمرين الأتمتة

### خريطة المضيفات الحتمية وإصدار دليل RAD

1. تشغيل مساعد اشتقاق المضيفات الحتمي على مجموعة manifeste المقترحة والتأكد
   من عدم وجود drift مقارنةً بـ
   `docs/source/soradns/deterministic_hosts.md`.
2. Utilisez ces résolveurs :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. تدوين معرّف الدليل المطبوع وSHA-256 ومسارات الإخراج داخل
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` et محاضر الانطلاقة.

### DNS تليمترية

- Nombre de résolveurs لمدة ≥10 دقائق عبر
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Utilisez Pushgateway et NDJSON pour utiliser l'ID d'exécution.

### تمارين أتمتة passerelle

1. Utiliser TLS/ECH :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. تشغيل أداة التوافق (`ci/check_sorafs_gateway_conformance.sh`) et auto-certification
   (`scripts/sorafs_gateway_self_cert.sh`) Pour utiliser Norito.
3. La fonction PagerDuty/Webhook est une solution de bout en bout.

### تجميع الأدلة- تحديث `ops/drill-log.md` pour la sonde et la sonde.
- Vous pouvez utiliser l'ID d'exécution et le lien vers Docs/DevRel.
- ربط حزمة الأدلة في تذكرة الحوكمة قبل مراجعة الانطلاقة.

## إدارة الجلسة وتسليم الأدلة

- **الخط الزمني للمشرف :**
  - T-24 h — ينشر Program Management التذكير + لقطة الأجندة/الحضور في `#nexus-steering`.
  - T-2 h — يقوم Networking TL pour `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation pour les sondes et l'ID d'exécution pour `artifacts/sorafs_gateway_dns/current`.
  - أثناء المكالمة — يشارك المشرف هذا الدليل ويُعيّن ناسخًا مباشرًا؛ وتلتقط Docs/DevRel بنود العمل أثناء الجلسة.
- **قالب المحاضر:** انسخ الهيكل من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (pour le bundle de produits)
  والتزم بإيداع نسخة مكتملة لكل جلسة. تضمّن قائمة الحضور والقرارات وبنود العمل
  وهاشات الأدلة والمخاطر المفتوحة.
- **رفع الأدلة:** اضغط مجلد `runbook_bundle/` الخاص بالتمرين، أرفق PDF المحاضر
  مُصدّر، وسجّل هاشات SHA-256 في المحاضر + الأجندة، ثم نبّه اسم المراجعين
  La référence est `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة الأدلة (انطلاقة مارس 2025)

آخر الآرتيفاكتات المرتبطة بالخارطة والمحاضر محفوظة في
`s3://sora-governance/sorafs/gateway_dns/`. الهاشات أدناه تعكس
الـ manifeste المعتمد (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Exécution à sec — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball الحزمة: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Fichier PDF : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة مباشرة — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(رفع متوقع: `gateway_dns_minutes_20250303.pdf` — ستضيف Docs/DevRel قيمة SHA-256 عند توفر PDF في الحزمة.)_

## مواد ذات صلة

- [passerelle دليل تشغيل عمليات](./operations-playbook.md)
- [SoraFS](./observability-plan.md)
- [passerelle DNS de la passerelle](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)