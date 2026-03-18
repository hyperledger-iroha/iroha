---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تشغيل انطلاقة Gateway e DNS em SoraFS

تعكس نسخة البوابة هذا الدليل التشغيلي المعتمد الموجود في
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
وهي تلتقط الضوابط التشغيلية لمسار DNS descentralizado e gateway em فرق
As datas de lançamento e lançamento de 2025-03.

## النطاق والمخرجات

- ربط معالم DNS (SF-4) e gateway (SF-5) عبر تمارين اشتقاق المضيفات الحتمي,
  Você pode usar resolvedores, TLS/GAR e TLS/GAR.
- إبقاء مدخلات الانطلاقة (الأجندة, الدعوة, متعقب الحضور, لقطة تليمترية GAR)
  متزامنة مع آخر تعيينات المالكين.
- إنتاج حزمة آرتيفاكتات قابلة للتدقيق لمراجعي الحوكمة: ملاحظات إصدار دليل
  resolvedores, سجلات فحوصات gateway, مخرجات أداة التوافق, وملخص Docs/DevRel.

## الأدوار والمسؤوليات

| المسار | Produtos | Máquinas de lavar louça |
|--------|-------------|-----------------------|
| Rede TL (DNS) | Você pode usar o RAD para resolver os problemas do RAD. | `artifacts/soradns_directory/<ts>/`, modelo `docs/source/soradns/deterministic_hosts.md`, e código RAD. |
| Líder de Automação de Operações (gateway) | Você pode usar TLS/ECH/GAR, usar `sorafs-gateway-probe` e usar PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON para sonda, e `ops/drill-log.md`. |
| Grupo de controle de qualidade e ferramentas | Você pode usar `ci/check_sorafs_gateway_conformance.sh`, luminárias fixas, e usar o autocertificador para Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | تدوين المحاضر, تحديث pré-ler التصميم + الملاحق, ونشر ملخص الأدلة في هذا البوابة. | Verifique `docs/source/sorafs_gateway_dns_design_*.md` o número de telefone e o número de telefone. |

## المدخلات والمتطلبات المسبقة

- مواصفة المضيفات الحتمية (`docs/source/soradns/deterministic_hosts.md`) وبنية
  Resolver resolvedores (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway de gateway: دليل المشغل, مساعدات أتمتة TLS/ECH, modo direto,
  O autocertificado é `docs/source/sorafs_gateway_*`.
- Nome: `cargo xtask soradns-directory-release`;
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e CI `scripts/sorafs_gateway_self_cert.sh`, e CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Nome: مفتاح إصدار GAR, بيانات اعتماد ACME para DNS/TLS, مفتاح توجيه PagerDuty,
  Eu uso Torii para resolver resolvedores.

## قائمة التحقق قبل التنفيذ

1. تأكيد الحضور والأجندة بتحديث
   `docs/source/sorafs_gateway_dns_design_attendance.md` é um recurso de teste
   (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. تجهيز جذور الآرتيفاكتات مثل
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. تحديث fixtures (manifests الخاصة بـ GAR, أدلة RAD, حزم توافق gateway) مع التأكد
   No caso de `git submodule`, você pode remover o produto e removê-lo.
4. من الأسرار (مفتاح إصدار Ed25519, ملف حساب ACME, رمز PagerDuty) e ومطابقة
   somas de verificação no vault.
5. Faça o teste de fumaça no site (endpoint no Pushgateway, no GAR no Grafana)
   قبل التمرين.

## خطوات تمرين الأتمتة

### خريطة المضيفات الحتمية وإصدار دليل RAD

1. تشغيل مساعد اشتقاق المضيفات الحتمي على مجموعة manifestos المقترحة والتأكد
   من عدم وجود drift مقارنةً بـ
   `docs/source/soradns/deterministic_hosts.md`.
2. Para definir os resolvedores:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Verifique o valor do arquivo e SHA-256 e instale-o.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` é um código de erro.

### Nome do DNS- تتبع سجلات شفافية resolvers لمدة ≥10 دقائق عبر
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- تصدير مقاييس Pushgateway وأرشفة لقطات NDJSON بجانب مجلد run ID.

### تمارين أتمتة gateway

1. Configuração do TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. تشغيل أداة التوافق (`ci/check_sorafs_gateway_conformance.sh`) e autocertificação
   (`scripts/sorafs_gateway_self_cert.sh`) é um dispositivo de substituição Norito.
3. Use o PagerDuty/Webhook para fazer o download de ponta a ponta.

### تجميع الأدلة

- Verifique `ops/drill-log.md` para testar a sonda e a sonda.
- تخزين الآرتيفاكتات ضمن مجلدات run ID ونشر ملخص تنفيذي ضمن محاضر Docs/DevRel.
- ربط حزمة الأدلة في تذكرة الحوكمة قبل مراجعة الانطلاقة.

## إدارة الجلسة وتسليم الأدلة

- **الخط الزمني للمشرف:**
  - T-24 h — ينشر Program Management التذكير + لقطة الأجندة/الحضور em `#nexus-steering`.
  - T-2 h — يقوم Networking TL بتحديث لقطة تليمترية GAR وتسجيل الفروقات em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation através de probes e ID de execução em `artifacts/sorafs_gateway_dns/current`.
  - أثناء المكالمة — يشارك المشرف هذا الدليل ويُعيّن ناسخًا مباشرًا؛ Acesse Docs/DevRel para obter mais informações.
- **قالب المحاضر:** انسخ الهيكل من
  `docs/source/sorafs_gateway_dns_design_minutes.md` (especificado no pacote do pacote)
  والتزم بإيداع نسخة مكتملة لكل جلسة. تضمّن قائمة الحضور والقرارات وبنود العمل
  وهاشات الأدلة والمخاطر المفتوحة.
- **رفع الأدلة:** اضغط مجلد `runbook_bundle/` الخاص بالتمرين, أرفق PDF المحاضر
  O código de barras SHA-256 é o código + código de barras do SHA-256.
  O número de telefone é `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## لقطة الأدلة (março de 2025)

آخر الآرتيفاكتات المرتبطة بالخارطة والمحاضر محفوظة في
`s3://sora-governance/sorafs/gateway_dns/`. الهاشات أدناه تعكس
O manifesto do código (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Teste — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Nome do Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF Código: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ورشة مباشرة — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(رفع متوقع: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel قيمة SHA-256 عند توفر PDF no site.)_

## مواد ذات صلة

- [Gateway de gateway](./operations-playbook.md)
- [SoraFS](./observability-plan.md)
- [nome do DNS e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)