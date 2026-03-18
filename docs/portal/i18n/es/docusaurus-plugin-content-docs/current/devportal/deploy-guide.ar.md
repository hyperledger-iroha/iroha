---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## نظرة عامة

Utilice el conector **DOCS-7** (SoraFS) y **DOCS-8** (pin para CI/CD) y **DOCS-8** لبوابة المطورين. Primero use build/lint, use SoraFS y use Sigstore, y luego use build/lint. وتمارين التراجع بحيث تكون كل معاينة واصدار قابلة لاعادة الانتاج وقابلة للتدقيق.

Utilice el punto final `sorafs_cli` (en lugar de `--features cli`) y el punto final Torii. registro-PIN, y están disponibles OIDC y Sigstore. خزّن الاسرار طويلة العمر (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, رموز Torii) في خزينة CI؛ También hay exportaciones de shell.

## المتطلبات المسبقة- Nodo 18.18 entre `npm` y `pnpm`.
- `sorafs_cli` de `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان Torii يكشف `/v1/sorafs/*` مع حساب/مفتاح خاص بسلطة يمكنها ارسال المانيفست والاسماء المستعارة.
- مُصدِر OIDC (GitHub Actions, GitLab, identidad de carga de trabajo, الخ) لاصدار `SIGSTORE_ID_TOKEN`.
- Contenido: `examples/sorafs_cli_quickstart.sh` flujos de trabajo de flujo de trabajo y `docs/source/sorafs_ci_templates.md` de flujos de trabajo en GitHub/GitLab.
- اضبط متغيرات OAuth الخاصة بـ Pruébelo (`DOCS_OAUTH_*`) y
  [lista de verificación de refuerzo de seguridad](./security-hardening.md) قبل ترقية build خارج المختبر. بناء البوابة يفشل الان عند غياب هذه المتغيرات او عند خروج مفاتيح TTL/sondeo عن النوافذ المفروضة؛ Utilice `DOCS_OAUTH_ALLOW_INSECURE=1` para eliminar el agua. ارفق ادلة اختبار الاختراق مع تذكرة الاصدار.

## الخطوة 0 - التقاط حزمة وكيل Pruébalo

Haga clic en Netlify y haga clic en Pruébelo y en OpenAPI para obtener el siguiente enlace:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

Aquí `scripts/tryit-proxy-release.mjs` utiliza proxy/probe/rollback y conecta OpenAPI y `release.json` con `checksums.sha256`. Utilice la puerta de enlace Netlify/SoraFS para acceder a la puerta de enlace de Netlify/SoraFS. وتلميحات Torii بدون اعادة بناء. تسجّل الحزمة ايضا ما اذا كان تم تمكين tokens al portador المرسلة من العميل (`allow_client_auth`) للحفاظ على اتساق خطة الاطلاق También CSP.

## الخطوة 1 - بناء وفحص pelusa للبوابة

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```Nombre `npm run build` Nombre `scripts/write-checksums.mjs` y:

- `build/checksums.sha256` - Adaptador SHA256 para `sha256sum -c`.
- `build/release.json` - بيانات وصفية (`tag`, `generated_at`, `source`) مثبتة في كل CAR/مانيفست.

احفظ كلا الملفين مع ملخص CAR حتى يتمكن المراجعون من مقارنة نواتج المعاينة بدون اعادة بناء.

## الخطوة 2 - تغليف الاصول الثابتة

Empaquetadora de automóviles para el hogar Docusaurus. La configuración del sistema es `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Hay fragmentos JSON, resúmenes y pruebas de prueba con `manifest build` y archivos CI.

## الخطوة 2b - تغليف مرافقات OpenAPI y SBOM

Los controladores DOCS-7 y OpenAPI y SBOM se conectan a través de dispositivos inalámbricos. `Sora-Proof`/`Sora-Content-CID` es un artefacto. يقوم مساعد الاصدار (`scripts/sorafs-pin-release.sh`) بالفعل بتغليف مجلد OpenAPI (`static/openapi/`) y SBOM الناتجة عبر `syft` para COCHES منفصلة `openapi.*`/`*-sbom.*` y البيانات في `artifacts/sorafs/portal.additional_assets.json`. عند اتباع المسار اليدوي، اعد الخطوات 2-4 لكل payload مع بادئاتها ووسوم metadata الخاصة بها (مثلا `--car-out "$OUT"/openapi.car` مع `--metadata alias_label=docs.sora.link/openapi`). Utilice el manifiesto/alias Torii (OpenAPI, SBOM البوابة, SBOM OpenAPI) para configurar DNS البوابة من تقديم pruebas مدبسة لكل artefacto منشور.

## الخطوة 3 - بناء المانيفست

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```Esta es la política de pin de la aplicación `--pin-storage-class hot`. Utilice el formato JSON para crear archivos.

## الخطوة 4 - التوقيع عبر Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Este paquete contiene muchos fragmentos y BLAKE3 junto con el token OIDC de JWT. احتفظ بالـ paquete والتوقيع المنفصل؛ يمكن لعمليات الترقية في الانتاج اعادة استخدام نفس الاصول بدون اعادة توقيع. يمكن للعمليات المحلية استبدال اعلام المزود بـ `--identity-token-env` (او تعيين `SIGSTORE_ID_TOKEN` في البيئة) عندما يصدر مساعد OIDC خارجي التوكن.

## الخطوة 5 - الارسال الى registro de pines

ارسل المانيفست الموقّع (وخطة الـ trozos) الى Torii. اطلب دائما resumen كي تبقى نتيجة التسجيل/الاسم المستعار قابلة للتدقيق.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Ver más (`docs-preview.sora`) الانتاج.

Los componentes principales son: `--alias-namespace`, `--alias-name` y `--alias-proof`. ينتج فريق الحوكمة prueba de paquete (base64 y Norito bytes) عند الموافقة؛ خزنه ضمن اسرار CI وقدمه كملف قبل استدعاء `manifest submit`. اترك اعلام alias فارغة اذا كنت تريد تثبيت المانيفست بدون تغيير DNS.

## الخطوة 5b - توليد مقترح حوكمة

يجب ان يرافق كل مانيفست مقترح جاهز للبرلمان بحيث يستطيع اي مواطن من Sora تقديم التغيير دون امتلاك بيانات اعتماد مميزة. بعد خطوات enviar/firmar نفّذ:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```يلتقط `portal.pin.proposal.json` El nombre del archivo `RegisterPinManifest` incluye fragmentos y alias. ارفقه مع تذكرة الحوكمة او بوابة البرلمان كي يتمكن المندوبون من مراجعة الـ payload دون اعادة بناء. هذا الامر لا يلمس مفتاح Torii, لذلك يمكن لاي مواطن ان ينشئ المقترح محليا.

## الخطوة 6 - التحقق من pruebas والقياسات

بعد التثبيت، نفذ خطوات التحقق الحتمية:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- راقب `torii_sorafs_gateway_refusals_total` e `torii_sorafs_replication_sla_total{outcome="missed"}` لاكتشاف الشذوذ.
- Utilice `npm run probe:portal` para probar y probar-it y para obtener más información.
- اجمع ادلة المراقبة المذكورة في
  [Publicación y seguimiento](./publishing-monitoring.md) لتلبية بوابة الملاحظة DOCS-3c. يدعم المساعد الان عدة `bindings` (الموقع، OpenAPI, SBOM للبوابة, SBOM لـ OpenAPI) y `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` على المضيف المستهدف عبر حارس `hostname`. Archivos JSON y archivos adjuntos (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) مجلد الاصدار:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الخطوة 6a - تخطيط شهادات البوابة

Utilice TLS SAN/challenge para configurar GAR y configurar DNS. Utilice el comodín DG-3 y el comodín SAN y Pretty-host y DNS-01 y los siguientes:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```قم بضم JSON مع حزمة الاصدار (او ارفقه مع تذكرة التغيير) حتى يستطيع المشغلون لصق قيم SAN داخل اعدادات `torii.sorafs_gateway.acme` ويتأكد مراجعو GAR من الخرائط canonical/pretty دون اعادة الحساب. Lea `--name` para que el dispositivo funcione correctamente.

## الخطوة 6b - اشتقاق خرائط المضيفين القانونية

قبل توليد قوالب GAR، سجّل خريطة المضيف الحتمية لكل alias. Incluye `cargo xtask soradns-hosts` y `--name` y un comodín (`*.gw.sora.id`). ويشتق المضيف الجميل (`<alias>.gw.sora.name`). احفظ الناتج ضمن artefactos الاصدار كي يتمكن مراجعو DG-3 من مقارنة الخريطة مع ارسال GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilice `--verify-host-patterns <file>` para crear un archivo de configuración GAR y JSON. يقبل المساعد عدة ملفات للتحقق، مما يسهل تدقيق قالب GAR e `portal.gateway.binding.json` في نفس الاستدعاء:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Configuración de JSON y configuración de DNS/البوابة حتى يستطيع المدققون تأكيد المضيفين القانونية/الجميلة دون اعادة تشغيل السكربتات. اعد تشغيل الامر عند اضافة alias جديد حتى ترث تحديثات GAR نفس الادلة.

## الخطوة 7 - Cambio de DNS y cambio de DNS

تتطلب عمليات القطع في الانتاج حزمة تغيير قابلة للتدقيق. بعد نجاح الارسال (ربط الاسم المستعار), يصدر المساعد
`artifacts/sorafs/portal.dns-cutover.json` ، ويتضمن:- بيانات ربط الاسم المستعار (espacio de nombres/nombre/prueba, بصمة المانيفست، عنوان Torii, época الارسال، السلطة).
- سياق الاصدار (etiqueta, etiqueta de alias, مسارات المانيفست/CAR, خطة الـ trozos, paquete Sigstore).
- مؤشرات التحقق (sonda de sonda, alias + punto final Torii).
- حقول تغيير اختيارية (معرف التذكرة، نافذة cutover, جهة الاتصال التشغيلية, مضيف/منطقة الانتاج).
- بيانات ترقية المسار المشتقة من رأس `Sora-Route-Binding` (المضيف القانوني/CID, مسارات الرأس + الربط، اوامر التحقق) لضمان ان ترقية GAR وتمارين التراجع تشير لنفس الادلة.
- ملفات plan de ruta المولدة (`gateway.route_plan.json`، قوالب الرؤوس، ورؤوس التراجع الاختيارية) حتى تحقق تذاكر DG-3 و lint في CI من خطط الترقية/التراجع.
- Archivos de configuración de archivos (purga de archivos, autenticación de archivos JSON y `curl`).
- تلميحات التراجع التي تشير الى الوصف السابق (وسم الاصدار وبصمة المانيفست) لابقاء المسار الحتمي للتراجع.

عند الحاجة لعمليات purge, ولّد خطة قانونية بجانب الوصف:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

ارفق `portal.cache_plan.json` بحزمة DG-3 ليحصل المشغلون على مسارات/مضيفين حتميين (ومرشحات auth) عند ارسال طلبات `PURGE`. يمكن للقسم الاختياري في الوصف ان يشير الى هذا الملف مباشرة.

تحتاج حزمة DG-3 ايضا الى قائمة فحص للترقية والتراجع. Aquí está `cargo xtask soradns-route-plan`:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```يسجل `gateway.route_plan.json` المضيفين القانونيين/الجميلين، تذكيرات فحوص الصحة المرحلية, تحديثات الربط GAR, Purge للكاش، وخطوات التراجع. ارفقه مع ملفات GAR/binding/cutover قبل ارسال التذكرة حتى يتمكن فريق Ops من التدريب على الخطوات نفسها.

Este `scripts/generate-dns-cutover-plan.mjs` está conectado a `sorafs-pin-release.sh`. للتوليد اليدوي:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

املأ البيانات الاختيارية عبر متغيرات البيئة قبل تشغيل المساعد:

| المتغير | الغرض |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف التذكرة المخزن في الوصف. |
| `DNS_CUTOVER_WINDOW` | Transferencia de imagen según ISO8601 (modelo `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | مضيف الانتاج + المنطقة المعتمدة. |
| `DNS_OPS_CONTACT` | جهة الاتصال للطوارئ. |
| `DNS_CACHE_PURGE_ENDPOINT` | نقطة purgue المخزنة في الوصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | متغير البيئة الذي يحمل token purga (الافتراضي `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | مسار الوصف السابق لبيانات التراجع. |

Configuración JSON de configuración DNS, configuración de configuración y configuración de sonda فحص سجلات CI. Configuración de CLI para `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact` y `--cache-purge-endpoint`. و`--cache-purge-auth-env` و`--previous-dns-plan` نفس امكانية التخصيص خارج CI.

## الخطوة 8 - اصدار هيكل archivo de zona للمحلل (اختياري)Puede utilizar un archivo de zona en SNS y un fragmento de código para crear archivos de zona. Configuración de DNS y configuración de CLI؛ سيستدعي المساعد `scripts/sns_zonefile_skeleton.py` مباشرة بعد توليد الوصف. O على الاقل قيمة A/AAAA/CNAME وبصمة GAR (BLAKE3-256 للحمولة الموقعة). Para el hogar/para el hogar y para el hogar, `--dns-zonefile-out`, para el hogar, para `artifacts/sns/zonefiles/<zone>/<hostname>.json` y para `ops/soradns/static_zones.<hostname>.json`. كـ fragmento de resolución.| المتغير / الخيار | الغرض |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | مسار esqueleto de archivo de zona الناتج. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Fragmento de resolución más grande (الافتراضي `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL للسجلات الناتجة (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | عناوين IPv4 (قائمة مفصولة بفواصل او علم متكرر). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | عناوين IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | هدف CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Adaptador SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | سجلات TXT اضافية (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | تجاوز etiqueta النسخة المحسوب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | La marca de tiempo `effective_at` (RFC3339) está disponible en el sitio web. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | تجاوز prueba المسجل في البيانات. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | تجاوز CID المسجل في البيانات. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | حالة تجميد الحارس (suave, duro, descongelación, seguimiento, emergencia). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | مرجع تذكرة التجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | marca de tiempo RFC3339 لمرحلة descongelación. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | ملاحظات تجميد اضافية (قائمة مفصولة بفواصل). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | بصمة BLAKE3-256 (hex) للحمولة الموقعة. مطلوبة عند وجود fijaciones للبوابة. |تقرأ GitHub Actions هذه القيم من اسرار المستودع حتى ينتج كل pin للانتاج artefactos تلقائيا. اضبط الاسرار التالية (يمكن ان تحتوي القيم على قوائم مفصولة بفواصل):

| Secreto | الغرض |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | مضيف/منطقة الانتاج للمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | جهة الاتصال المخزنة في الوصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Utilice el protocolo IPv4/IPv6. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | هدف CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Utilice SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | سجلات TXT اضافية. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | بيانات التجميد في الهيكل. |
| `DOCS_SORAFS_GAR_DIGEST` | بصمة BLAKE3 بالصيغة السداسية للحمولة الموقعة. |

Utilice `.github/workflows/docs-portal-sorafs-pin.yml`, y `dns_change_ticket` y `dns_cutover_window` para acceder a/zonefile. اتركها فارغة فقط لعمليات ejecución en seco.

مثال نموذجي (propietario del runbook del SN-7):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  ...other flags...
```

Para obtener más información, escriba TXT y escriba el mensaje `effective_at`. للمسار التشغيلي الكامل راجع `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### ملاحظة حول تفويض DNS العام

Este archivo de zona está disponible para todos los usuarios. ما زلت بحاجة لضبط تفويض NS/DS للمنطقة الام لدى المسجل او مزود DNS حتى يتمكن الانترنت العام من العثور على خوادم الاسماء.- لعمليات cutover عند apex/TLD استخدم ALIAS/ANAME (حسب المزود) او انشر سجلات A/AAAA تشير الى عناوين anycast الخاصة بالبوابة.
- للنطاقات الفرعية, انشر CNAME الى الـ Pretty Host المشتق (`<fqdn>.gw.sora.name`).
- المضيف القانوني (`<hash>.gw.sora.id`) يبقى تحت نطاق البوابة ولا يُنشر داخل نطاقك العام.

### قالب رؤوس البوابة

ينتج مساعد النشر ايضا `portal.gateway.headers.txt` و`portal.gateway.binding.json` وهما artefactos تلبي متطلبات DG-3 لربط محتوى البوابة:

- `portal.gateway.headers.txt` para conexión HTTP (para usar `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP y HSTS) و`Sora-Route-Binding`) والتي يجب على بوابات الحافة لصقها في كل رد.
- يسجل `portal.gateway.binding.json` المعلومات نفسها بشكل مقروء آليا حتى تتمكن تذاكر التغيير yالاتمتة من مقارنة host/cid دون كشط مخرجات shell.

يتم توليدها تلقائيا عبر `cargo xtask soradns-binding-template` وتلتقط alias وبصمة المانيفست وhostname الخاص بالبوابة الممرر الى `sorafs-pin-release.sh`. لاعادة التوليد او التخصيص:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

مرر `--csp-template` او `--permissions-template` او `--hsts-template` لتجاوز قوالب الرؤوس الافتراضية عند الحاجة، وادمجها مع مفاتيح `--no-*` لازالة رأس بالكامل.

Configuración de CDN y JSON para configuración de archivos y archivos JSON الاصدار.

يشغّل سكربت الاصدار مساعد التحقق تلقائيا حتى تحتوي تذاكر DG-3 على ادلة حديثة. Aquí está el texto JSON:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```Introduzca el nombre de host `Sora-Proof` y configure el nombre de host y el nombre de host de `Sora-Route-Binding`. اذا انحرف اي رأس. احتفظ بمخرجات الكونسول مع باقي artefactos عند تشغيل الامر خارج CI.

> **تكامل واصف DNS:** يدمج `portal.dns-cutover.json` الان قسما `gateway_binding` يشير الى هذه الملفات (المسارات وcontent CID وحالة والقالب النصي للرؤوس) **وكذلك** مقطع `route_plan` الذي يشير الى `gateway.route_plan.json` وقوالب الرؤوس الرئيسية/التراجع. ادرج هذه الاقسام في كل تذكرة DG-3 ليتمكن المراجعون من مقارنة قيم `Sora-Name/Sora-Proof/CSP` y التاكد من ان خطط الترقية/التراجع تطابق حزمة الادلة دون فتح الارشيف.

## الخطوة 9 - تشغيل مراقبات النشر

يتطلب بند خارطة الطريق **DOCS-3c** ادلة مستمرة على ان البوابة ووكيل Pruébelo y وروابط البوابة تظل سليمة بعد النشر. شغّل المراقب الموحد مباشرة بعد الخطوتين 7-8 واربطه بمسباراتك المجدولة:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- يقوم `scripts/monitor-publishing.mjs` بتحميل ملف الاعداد (راجع `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) وتنفيذ ثلاث فحوصات: مسارات البوابة مع تحقق CSP/Permisos-Política, مسبارات y كيل Pruébelo (اختياريا `/metrics`), y محقق ربط البوابة (`cargo xtask soradns-verify-binding`) الذي يفرض وجود قيمة Sora-Content-CID المتوقعة مع فحص alias/manifest.
- ينهي الامر بخروج غير صفري عند فشل اي probe حتى تتمكن CI/cron/فرق التشغيل من ايقاف الترقية.
- El archivo `--json-out` contiene archivos JSON y archivos و`--evidence-dir` يصدر `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` حتى يتمكن مراجعو الحوكمة من مقارنة النتائج دون اعادة التشغيل. Aquí está el paquete `artifacts/sorafs/<tag>/monitoring/`, el paquete Sigstore y el descriptor de transferencia de DNS.
- Dispositivos de alarma y alarma Grafana (`dashboards/grafana/docs_portal.json`) y Alertmanager Drill que funcionan con SLO en DOCS-3c قابلا للتدقيق لاحقا. Utilice el dispositivo `docs/portal/docs/devportal/publishing-monitoring.md`.

Utilice HTTPS y utilice `http://` para acceder a `allowInsecureHttp`. Asegúrese de que TLS esté disponible para dispositivos móviles y dispositivos electrónicos.

Este es el código `npm run monitor:publishing` de Buildkite/cron. نفس الامر مع روابط الانتاج يغذي فحوصات الصحة المستمرة بين الاصدارات.

## الاتمتة عبر `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` Pasos 2-6. فهو:1. يؤرشف `build/` en tarball حتمي،
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` y `proof verify`.
3. ينفذ `manifest submit` اختياريا (un enlace de alias) عندما تتوفر بيانات اعتماد Torii,
4. Introduzca `artifacts/sorafs/portal.pin.report.json` y `portal.pin.proposal.json` y el descriptor de transferencia de DNS y el descriptor de red (`portal.gateway.binding.json` para el servidor) Haga clic en el botón de encendido y en el botón de encendido de la tarjeta CI.

Utilice `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` y (اختياريا) `PIN_ALIAS_PROOF_PATH` para evitar errores. Fuente de alimentación `--skip-submit` Este flujo de trabajo en GitHub está disponible en `perform_submit`.

## الخطوة 8 - نشر مواصفات OpenAPI y SBOM

Utilice DOCS-7 y los archivos OpenAPI y SBOM. تغطي الادوات الحالية الثلاثة:

1. **اعادة توليد وتوقيع المواصفات.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Utilice `--version=<label>` para conectar el dispositivo (`2025-q3`). Adaptador de corriente `static/openapi/versions/<label>/torii.json` y `versions/current` y `static/openapi/versions.json` SHA-256 y المانيفست وتاريخ التحديث. يقرأ موقع المطورين هذا المؤشر ليعرض اختيار النسخة ومعلومات البصمة/التوقيع. Utilice `--version` para conectar los cables y `current` y ​​`latest`.

   Los dispositivos SHA-256/BLAKE3 están conectados entre `Sora-Proof` y `/reference/torii-swagger`.2. **اصدار SBOM CycloneDX.** يتوقع مسار الاصدار SBOMs عبر syft حسب `docs/source/sorafs_release_pipeline_plan.md`. احتفظ بالمخرجات قرب artefactos البناء:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **تغليف كل carga útil en el COCHE.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   اتبع نفس خطوات `manifest build` / `manifest sign` الخاصة بالموقع الرئيسي، y alias لكل artefacto (مثل `docs-openapi.sora` للمواصفات و`docs-sbom.sora` لحزمة SBOM الموقعة). Hay SoraDNS y GAR para crear una carga útil.

4. **الارسال والربط.** اعادة استخدام autoridad y Sigstore paquete, مع تسجيل alias tupla في قائمة فحص الاصدار حتى يعرف المدققون اي اسم Sora يقابل اي بصمة.

يساعد حفظ مانيifestات OpenAPI/SBOM مع بناء البوابة على ان يحتوي كل ticket على مجموعة الادلة الكاملة دون اعادة تشغيل empacador.

### مساعد الاتمتة (CI/script de paquete)

`./ci/package_docs_portal_sorafs.sh` يجمع الخطوات 1-8 في امر واحد. يقوم المساعد بـ:

- اعداد البوابة (`npm ci`, مزامنة OpenAPI/Norito, اختبارات widgets).
- اصدار CARs ومانيفستات للبوابة و OpenAPI و SBOM عبر `sorafs_cli`.
- `sorafs_cli proof verify` (`--proof`) y Sigstore (`--sign`, `--sigstore-provider`, `--sigstore-audience`) اختياريا.
- Utilice `artifacts/devportal/sorafs/<timestamp>/` y `package_summary.json` para CI/release.
- تحديث `artifacts/devportal/sorafs/latest` ليشير الى اخر تشغيل.

مثال (خط كامل مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

اهم الاعلام:- `--out <dir>` - تغيير جذر الاصول (الافتراضي مجلدات marca de tiempo).
- `--skip-build` - اعادة استخدام `docs/portal/build` الموجود.
- `--skip-sync-openapi` - تجاوز `npm run sync-openapi` عندما لا يستطيع `cargo xtask openapi` الوصول الى crates.io.
- `--skip-sbom` - عدم استدعاء `syft` عند عدم توفره (مع تحذير).
- `--proof` - تنفيذ `sorafs_cli proof verify` لكل زوج CAR/manifest.
- `--sign` - Actualización `sorafs_cli manifest sign`.

للانتاج استخدم `docs/portal/scripts/sorafs-pin-release.sh`. Este portal/OpenAPI/SBOM contiene información y metadatos de `portal.additional_assets.json`. يدعم مفاتيح `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` لتعيين alias لكل artefacto وتجاوز مصدر SBOM عبر `--openapi-sbom-source` وتخطي Cargas útiles (`--skip-openapi`/`--skip-sbom`) y `syft` o `--syft-bin`.

يعرض السكربت كل الاوامر؛ La configuración de `package_summary.json` incluye resúmenes y metadatos de shell.

## الخطوة 9 - التحقق من puerta de enlace + SoraDNS

Para obtener una transferencia de corte, use un alias de SoraDNS y pruebas de prueba:

1. **تشغيل sonda gate.** يقوم `ci/check_sorafs_gateway_probe.sh` بتشغيل `cargo xtask sorafs-gateway-probe` على accesorios في `fixtures/sorafs_gateway/probe_demo/`. Nombre de host y nombre de host:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Sonda de prueba `Sora-Name`, `Sora-Proof`, `Sora-Proof-Status`, `docs/source/sorafs_alias_policy.md` y otras aplicaciones TTL y enlaces GAR.Para controles puntuales ligeros (por ejemplo, cuando sólo el paquete de encuadernación
   cambiado), ejecute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   El asistente valida el paquete de enlaces capturado y es útil para su lanzamiento.
   boletos que solo necesitan confirmación vinculante en lugar de un simulacro de sonda completa.

2. **جمع ادلة taladros.** استخدم `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` لتجميع الرؤوس والسجلات في `artifacts/sorafs_gateway_probe/<stamp>/` وتحديث `ops/drill-log.md`، مع Utilice ganchos de reversión y PagerDuty. Utilice `--host docs.sora` para conectarse a SoraDNS.

3. **التحقق من enlaces DNS.** عند نشر prueba الخاص بالalias، احفظ ملف GAR المشار اليه وارفقة مع ادلة الاصدار. El solucionador de problemas es `tools/soradns-resolver`. Para usar JSON, utilice `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` para mapeo de host, metadatos y etiquetas de telemetría. يمكن للمساعد اصدار ملخص `--json-out` بجانب GAR الموقّع.
  عند صياغة GAR جديد، فضّل `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`، ولا تستخدم `--manifest-cid` الا عند غياب ملف المانيفست. Utilice CID y BLAKE3 para crear archivos JSON, etiquetas y etiquetas y etiquetas. CSP/HSTS/Política de permisos قبل الكتابة لضمان الحتمية.

4. **مراقبة مقاييس alias.** ابق `torii_sorafs_alias_cache_refresh_duration_ms` و`torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة؛ Utilice el `dashboards/grafana/docs_portal.json`.

## الخطوة 10 - المراقبة وحزم الادلة- **اللوحات.** Seleccione `dashboards/grafana/docs_portal.json`, `dashboards/grafana/sorafs_gateway_observability.json` y `dashboards/grafana/sorafs_fetch_observability.json` para que funcionen correctamente.
- **ارشفة sondas.** احتفظ بـ `artifacts/sorafs_gateway_probe/<stamp>/` ضمن git-annex او مخزن الادلة.
- **حزمة الاصدار.** خزّن ملخصات CAR, مجموعات مانيفست وتواقيع Sigstore و`portal.pin.report.json` وسجلات Try-It وتقارير الروابط تحت مجلد واحد بتاريخ.
- **سجل drills.** عند تشغيل drills, يحدث `scripts/telemetry/run_sorafs_gateway_probe.sh` ملف `ops/drill-log.md` لتلبية متطلبات SNNet-5.
- **روابط التذاكر.** اربط معرفات لوحات Grafana او صادرات PNG مع مسار تقرير sonda حتى يتمكن المراجعون من التحقق دون وصول cáscara.

## الخطوة 11 - تمرين buscar متعدد المصادر وادلة marcador

Utilice SoraFS para buscar el archivo DNS (DOCS-7/SF-6) en DNS/gateway. بعد تثبيت المانيفست:

1. **تشغيل `sorafs_fetch` على المانيفست الحي.** استخدم نفس plan/manifest مع بيانات اعتماد البوابة لكل مزود واحفظ جميع المخرجات:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - احصل اولا على anuncios de proveedores المشار اليها في المانيفست (مثل `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) y مررها عبر `--provider-advert name=path` لضمان تقييم القدرات بشكل حتمي. استخدم `--allow-implicit-provider-metadata` **فقط** عند اعادة تشغيل accesorios في CI.
   - Haga clic en el nombre del usuario para obtener el caché/alias.

2. **ارشفة المخرجات.** Introduzca `scoreboard.json`, `providers.ndjson`, `fetch.json` y `chunk_receipts.ndjson` en el interior.3. **تحديث القياسات.** استورد المخرجات في لوحة **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) y `torii_sorafs_fetch_duration_ms`/`_failures_total`.

4. **فحص قواعد التنبيه.** شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` y مخرجات promtool.

5. **الدمج في CI.** فعّل خطوة `sorafs_fetch` عبر `perform_fetch_probe` في puesta en escena/producción لتوليد الادلة تلقائيا.

## الترقية والملاحظة والتراجع

1. **الترقية:** احتفظ بـ alias منفصلة لـ puesta en escena y producción. قم بالترقية عبر اعادة `manifest submit` بالمانيفست نفسه مع تبديل `--alias-namespace/--alias-name` الى alias الانتاج.
2. **المراقبة:** استخدم `docs/source/grafana_sorafs_pin_registry.json` y مسبارات البوابة الخاصة في `docs/portal/docs/devportal/observability.md`.
3. **التراجع:** لاعادة التراجع، اعد ارسال المانيفست السابق (او اسحب alias الحالي) عبر `sorafs_cli manifest submit --alias ... --retire`.

## قالب CI

يجب ان يتضمن الحد الادنى من خط الانابيب:

1. Build + lint (`npm ci`, `npm run build`, sumas de comprobación adicionales).
2. التغليف (`car pack`) وحساب المانيفست.
3. Tarjeta de identificación del token OIDC (`manifest sign`).
4. رفع الاصول (CAR, مانيفست، paquete, plan, resúmenes).
5. Registro de PIN de الارسال الى:
   - Solicitudes de extracción -> `docs-preview.sora`.
   - Etiquetas / فروع محمية -> ترقية alias الانتاج.
6. تشغيل sondas + بوابات التحقق من prueba قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` يربط هذه الخطوات لاصدارات يدوية. Este es el flujo de trabajo:- بناء/اختبار البوابة،
- تغليف build عبر `scripts/sorafs-pin-release.sh`, ،
- Paquete de manifiesto de توقيع/تحقق en GitHub OIDC,
- رفع CAR/manifiesto/paquete/plan/resúmenes de prueba كـ artefactos,
- (اختياريا) ارسال المانيفست وربط alias عند توفر secretos.

اضبط secretos/variables التالية قبل التشغيل:

| Nombre | الغرض |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Aquí Torii y luego `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف época المسجل مع الارسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | سلطة التوقيع لارسال المانيفست. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tupla المربوط عند `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | prueba de paquete للalias بترميز base64 (اختياري). |
| `DOCS_ANALYTICS_*` | نقاط análisis/sonda الحالية. |

شغّل الـ flujo de trabajo عبر واجهة Acciones:

1. Utilice `alias_label` (`docs.sora.link`), `proposal_alias` y `release_tag`.
2. Abra `perform_submit`, haga clic en el botón Torii (ejecución en seco) y haga clic en el alias.

يبقى `docs/source/sorafs_ci_templates.md` كمرجع لقوالب عامة, لكن flujo de trabajo الخاص بالبوابة هو المسار المفضل للاصدارات اليومية.

## قائمة التحقق- [] Nombre `npm run build`, `npm run test:*` y `npm run check:links`.
- [ ] Haga clic en `build/checksums.sha256` y `build/release.json`.
- [] توليد CAR وplan وmanifest وresumary تحت `artifacts/`.
- [] حفظ Paquete Sigstore والتوقيع المنفصل مع السجلات.
- [] حفظ `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json` عند الارسال.
- [ ] ارشفة `portal.pin.report.json` (و`portal.pin.proposal.json` عند توفرها).
- [] Seleccione `proof verify` y `manifest verify-signature`.
- [] تحديث لوحات Grafana y sondas Try-It.
- [] ارفاق ملاحظات التراجع (ID المانيفست السابق + alias de resumen) مع تذكرة الاصدار.