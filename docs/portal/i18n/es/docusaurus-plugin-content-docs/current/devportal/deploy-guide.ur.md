---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

یہ پلی بک روڈ میپ آئٹمز **DOCS-7** (SoraFS اشاعت) اور **DOCS-8**
(CI/CD پن آٹومیشن) کو ڈیولپر پورٹل کے لئے قابلِ عمل طریقہ کار میں بدلتی ہے۔
یہ build/lint مرحلہ، SoraFS پیکجنگ، Sigstore کے ذریعے مینی فیسٹ سائننگ،
alias پروموشن، توثیق، اور rollback ڈرلز کو کور کرتی ہے تاکہ ہر vista previa اور
lanzamiento آرٹیفیکٹ قابلِ اعادہ اور قابلِ آڈٹ ہو۔

یہ فلو فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری (`--features cli` کے ساتھ
build شدہ) ہے، pin-registry اجازتوں والے Torii endpoint تک رسائی ہے، اور
Sigstore کے لئے OIDC اسناد ہیں۔ طویل مدتی راز (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii ٹوکنز) کو اپنے CI y میں رکھیں؛ لوکل رنز انہیں
exportaciones de conchas سے لوڈ کر سکتے ہیں۔

## پیشگی شرائط- Nodo 18.18+ کے ساتھ `npm` یا `pnpm`.
- `sorafs_cli` o `cargo run -p sorafs_car --features cli --bin sorafs_cli`, todos ellos
- Torii URL y `/v1/sorafs/*` ظاہر کرے اور ایک اتھارٹی اکاؤنٹ/پرائیویٹ کی جو
  مینی فیسٹس اور alias جمع کر سکے۔
- Emisor OIDC (GitHub Actions, GitLab, identidad de carga de trabajo y otros)
  `SIGSTORE_ID_TOKEN` منٹ کیا جا سکے۔
- Actualización: simulacros en `examples/sorafs_cli_quickstart.sh` en GitHub/GitLab
  flujos de trabajo کے لئے `docs/source/sorafs_ci_templates.md`.
- Pruébelo OAuth ویری ایبلز (`DOCS_OAUTH_*`) کنفیگر کریں اور build کو لیب سے باہر
  پروموٹ کرنے سے پہلے [lista de verificación de refuerzo de seguridad](./security-hardening.md)
  چلائیں۔ اب اگر یہ ویری ایبلز موجود نہ ہوں یا TTL/perillas de sondeo نافذ شدہ
  حدود سے باہر ہوں تو پورٹل build ناکام ہو جاتا ہے؛ `DOCS_OAUTH_ALLOW_INSECURE=1`
  صرف عارضی لوکل vistas previas کے لئے exportar کریں۔ prueba de penetración ثبوت ریلیز ٹکٹ کے ساتھ
  منسلک کریں۔

## مرحلہ 0 — Pruébalo پروکسی بنڈل محفوظ کریں

Netlify یا gateway پر vista previa پروموٹ کرنے سے پہلے Pruébelo پروکسی سورسز اور
Aquí está OpenAPI مینی فیسٹ digest کو ایک متعین بنڈل میں اسٹیمپ کریں:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` پروکسی/probe/rollback helpers کاپی کرتا ہے،
Firma OpenAPI کی تصدیق کرتا ہے، اور `release.json` کے ساتھ
`checksums.sha256` لکھتا ہے۔ Esta es la puerta de enlace Netlify/SoraFS.
ساتھ منسلک کریں تاکہ ریویورز عین وہی پروکسی سورسز اور Torii ٹارگٹ ہنٹس بغیر
دوبارہ construir کے ری پلے کر سکیں۔ بنڈل یہ بھی ریکارڈ کرتا ہے کہ آیا proporcionado por el cliente
portadores فعال تھے (`allow_client_auth`) تاکہ implementación پلان اور CSP قواعد ہم آہنگ
رہیں۔

## مرحلہ 1 — پورٹل build اور pelusa

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` خودکار طور پر `scripts/write-checksums.mjs` چلاتا ہے، اور
یہ تیار کرتا ہے:

- `build/checksums.sha256` — `sha256sum -c` کے لئے موزوں SHA256 مینی فیسٹ۔
- `build/release.json` — میٹا ڈیٹا (`tag`, `generated_at`, `source`) جو ہر
  CAR/manifiesto میں پِن کیا جاتا ہے۔

دونوں فائلیں CAR سمری کے ساتھ آرکائیو کریں تاکہ ریویورز بغیر دوبارہ build کیے
vista previa آرٹیفیکٹس کا فرق دیکھ سکیں۔

## مرحلہ 2 — اسٹیٹک اثاثوں کی پیکجنگ

CAR پیکر کو Docusaurus آؤٹ پٹ ڈائریکٹری پر چلائیں۔ نیچے کی مثال تمام آرٹیفیکٹس
`artifacts/devportal/` کے تحت لکھتی ہے۔

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

Recuentos de fragmentos JSON, resúmenes, sugerencias de planificación de pruebas
`manifest build` اور CI ڈیش بورڈز بعد میں دوبارہ استعمال کرتے ہیں۔

## مرحلہ 2b — OpenAPI اور SBOM معاون پیکج کریںDOCS-7 incluye una copia de seguridad de OpenAPI y una carga útil de SBOM
الگ الگ مینی فیسٹس کے طور پر شائع کیا جائے تاکہ gateways ہر آرٹیفیکٹ کے لئے
`Sora-Proof`/`Sora-Content-CID` ہیڈرز اسٹپل کر سکیں۔ ریلیز ہیلپر
(`scripts/sorafs-pin-release.sh`) پہلے ہی OpenAPI ڈائریکٹری (`static/openapi/`)
اور `syft` سے بننے والے SBOMs کو علیحدہ `openapi.*`/`*-sbom.*` CARs میں پیک
کرتا ہے اور میٹا ڈیٹا `artifacts/sorafs/portal.additional_assets.json` میں
ریکارڈ کرتا ہے۔ دستی فلو چلاتے وقت، ہر payload کے لئے مراحل 2-4 دہرائیں اور
اس کے اپنے prefijos اور etiquetas de metadatos استعمال کریں (مثال کے طور پر
`--car-out "$OUT"/openapi.car` کے ساتھ `--metadata alias_label=docs.sora.link/openapi`).
Nombre del DNS: manifiesto/alias Torii
OpenAPI, پورٹل SBOM, OpenAPI SBOM) تاکہ gateway تمام شائع شدہ آرٹیفیکٹس کے لئے
pruebas grapadas فراہم کر سکے۔

## مرحلہ 3 — مینی فیسٹ بنائیں

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

política de pin
canarios کے لئے `--pin-storage-class hot`)۔ Revisión de código JSON ورژن اختیاری ہے مگر
کے لئے سہولت دیتا ہے۔

## مرحلہ 4 — Sigstore سے سائن کریں

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```paquete de resumen, resumen de fragmentos, OIDC con hash BLAKE3
کرتا ہے بغیر JWT محفوظ کیے۔ paquete اور firma separada دونوں سنبھال کر
رکھیں؛ promociones de producción انہی آرٹیفیکٹس کو دوبارہ سائن کیے بغیر استعمال
کر سکتی ہیں۔ Proveedor de software فلیگز کو `--identity-token-env` سے بدل سکتے
ہیں (یا ماحول میں `SIGSTORE_ID_TOKEN` سیٹ کر سکتے ہیں) جب کوئی بیرونی OIDC
ہیلپر ٹوکن جاری کرے۔

## مرحلہ 5 — pin رجسٹری میں جمع کرائیں

سائن شدہ مینی فیسٹ (اور fragment plan) Torii کو جمع کرائیں۔ ہمیشہ resumen طلب کریں
تاکہ رجسٹری prueba de entrada/alias قابلِ آڈٹ رہے۔

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

جب vista previa یا alias canario (`docs-preview.sora`) رول آؤٹ کریں تو منفرد alias کے
ساتھ envío دوبارہ کریں تاکہ Control de calidad producción پروموشن سے پہلے مواد کی تصدیق کر سکے۔

Enlace de alias کے لئے تین فیلڈز درکار ہیں: `--alias-namespace`, `--alias-name`,
Nombre `--alias-proof`۔ گورننس alias درخواست منظور ہونے پر paquete de prueba (base64 یا
Norito bytes) تیار کرتی ہے؛ اسے CI secretos میں رکھیں اور `manifest submit`
چلانے سے پہلے اسے فائل کی صورت میں پیش کریں۔ جب آپ صرف مینی فیسٹ pin کرنا
چاہیں اور DNS کو ہاتھ نہ لگانا ہو تو alias فلیگز خالی چھوڑ دیں۔

## مرحلہ 5b — گورننس پروپوزل تیار کریں

ہر مینی فیسٹ کے ساتھ پارلیمنٹ کے لئے تیار پروپوزل ہونا چاہیے تاکہ کوئی بھی
Sora شہری خصوصی اسناد ادھار لئے بغیر تبدیلی پیش کر سکے۔ enviar/firmar مراحل کے
بعد چلائیں:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
````portal.pin.proposal.json` کینونیکل `RegisterPinManifest` ہدایت, resumen de fragmentos,
پالیسی، اور pista de alias کو محفوظ کرتا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ پورٹل کے
ساتھ منسلک کریں تاکہ نمائندے آرٹیفیکٹس دوبارہ build کیے بغیر payload کا فرق
دیکھ سکیں۔ چونکہ یہ کمانڈ Torii clave de autoridad کو کبھی نہیں چھوتی، کوئی بھی
شہری لوکل طور پر پروپوزل ڈرافٹ کر سکتا ہے۔

## مرحلہ 6 — pruebas اور ٹیلیمیٹری کی تصدیق

fijación کے بعد درج ذیل verificación determinista مراحل چلائیں:

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

- `torii_sorafs_gateway_refusals_total`
  `torii_sorafs_replication_sla_total{outcome="missed"}` میں anomalías چیک کریں۔
- `npm run probe:portal` چلائیں تاکہ Try-It پروکسی اور ریکارڈ شدہ لنکس کو
  نئے pin شدہ مواد کے خلاف پرکھا جا سکے۔
- [Publicación y seguimiento](./publishing-monitoring.md) میں بیان کردہ مانیٹرنگ
  شواہد حاصل کریں تاکہ DOCS-3c کا observabilidad گیٹ اشاعت کے مراحل کے ساتھ
  پورا ہو۔ ہیلپر اب متعدد Entradas `bindings` (سائٹ، OpenAPI, پورٹل SBOM, OpenAPI
  SBOM) قبول کرتا ہے اور اختیاری `hostname` گارڈ کے ذریعے ہدف host پر
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` نافذ کرتا ہے۔ نیچے والی invocación
  ایک JSON سمری اور paquete de evidencia (`portal.json`, `tryit.json`, `binding.json`,
  اور `checksums.sha256`) دونوں ریلیز ڈائریکٹری میں لکھتی ہے:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6a — puerta de enlace سرٹیفکیٹس کی منصوبہ بندیTLS SAN/challenge پلان GAR پیکٹس بنانے سے پہلے نکالیں تاکہ gateway ٹیم اور DNS
منظور کنندگان ایک ہی شواہد دیکھیں۔ نیا ہیلپر DG-3 آٹومیشن ان پٹس کو espejo کرتا
ہے، hosts comodín canónicos, SAN de host bonito, DNS-01 لیبلز، اور تجویز کردہ
ACME desafía a شمار کرتا ہے:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON کو ریلیز بنڈل کے ساتھ کمٹ کریں (یا چینج ٹکٹ کے ساتھ اپ لوڈ کریں) تاکہ
آپریٹرز Torii کی `torii.sorafs_gateway.acme` کنفیگ میں SAN ویلیوز چسپاں کر سکیں
اور GAR ریویورز asignaciones canónicas/bonitas کو derivaciones de host دوبارہ چلائے بغیر
تصدیق کر سکیں۔ اسی ریلیز میں پروموٹ ہونے والے ہر sufijo کے لئے اضافی `--name`
دلائل شامل کریں۔

## مرحلہ 6b — asignaciones de hosts canónicos اخذ کریں

Cargas útiles de GAR کے سانچوں سے پہلے ہر alias کے لئے mapeo determinista de host ریکارڈ
کریں۔ `cargo xtask soradns-hosts` ہر `--name` کو اس کے canónico لیبل
(`<base32>.gw.sora.id`) میں ہیش کرتا ہے، مطلوبہ comodín (`*.gw.sora.id`) نکالتا
ہے، اور host bonito (`<alias>.gw.sora.name`) اخذ کرتا ہے۔ ریلیز آرٹیفیکٹس میں
آؤٹ پٹ محفوظ کریں تاکہ DG-3 ریویورز Envío GAR کے ساتھ mapeo کا فرق دیکھ
سکیں:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب کوئی GAR یا enlace de puerta de enlace JSON مطلوبہ hosts میں سے کسی کو چھوڑ دے تو فوری
ناکامی کے لئے `--verify-host-patterns <file>` استعمال کریں۔ ہیلپر متعدد
verificación فائلیں قبول کرتا ہے، جس سے ایک ہی invocación میں plantilla GAR اور
grapado `portal.gateway.binding.json` دونوں کو pelusa کرنا آسان ہوتا ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```DNS/gateway چینج ٹکٹ کے ساتھ resumen JSON اور verificación لاگ منسلک کریں تاکہ
آڈیٹرز canónico, comodín, اور hosts bonitos کی تصدیق derivaciones de host دوبارہ
چلائے بغیر کر سکیں۔ جب بھی بنڈل میں نئے alias شامل ہوں تو یہ کمانڈ دوبارہ
چلائیں تاکہ بعد کے GAR اپڈیٹس میں وہی rastro de evidencia برقرار رہے۔

## مرحلہ 7 — Descriptor de transición de DNS تیار کریں

recortes de producción کے لئے آڈیٹ کے قابل تبدیلی پیکٹ درکار ہوتا ہے۔ کامیاب
envío (enlace de alias) کے بعد ہیلپر `artifacts/sorafs/portal.dns-cutover.json`
بناتا ہے، جس میں یہ شامل ہے:- metadatos de enlace de alias (espacio de nombres/nombre/prueba, resumen de manifiesto, URL Torii,
  época enviada, autoridad) ؛
- Etiquetas, etiquetas de alias, rutas de manifiesto/CAR, planes de fragmentos, paquete Sigstore)
- punteros de verificación (sonda کمانڈ، alias + punto final Torii)؛
- اختیاری control de cambios فیلڈز (identificación del ticket, ventana de transición, contacto de operaciones,
  nombre de host/zona de producción)؛
- grapado `Sora-Route-Binding` ہیڈر سے اخذ کردہ metadatos de promoción de ruta
  (host canónico/CID, encabezado + rutas de enlace, comandos de verificación), تاکہ GAR
  promoción اور simulacros de respaldo ایک ہی شواہد کا حوالہ دیں؛
- plan de ruta generado آرٹیفیکٹس (`gateway.route_plan.json`, plantillas de encabezado,
  اور اختیاری revertir encabezados) تاکہ cambiar tickets اور CI ganchos para pelusa ہر DG-3
  پیکٹ کے promoción/reversión canónica پلانز کا حوالہ منظوری سے پہلے دیکھ سکیں؛
- Metadatos de invalidación de caché (purgar punto final, variable de autenticación, carga útil JSON,
  اور مثال `curl` کمانڈ)؛ اور
- sugerencias de reversión جو پچھلے descriptor (etiqueta de lanzamiento اور resumen de manifiesto) کی طرف
  اشارہ کریں تاکہ cambiar boletos میں ruta de retorno determinista شامل ہو۔

جب ریلیز کو purga de caché درکار ہو تو descriptor de transición کے ساتھ ایک canónico
پلان بنائیں:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```حاصل شدہ `portal.cache_plan.json` کو DG-3 پیکٹ کے ساتھ منسلک کریں تاکہ آپریٹرز
کے پاس hosts/rutas deterministas (اور ملتے جلتے sugerencias de autenticación) ہوں جب وہ `PURGE`
solicitudes بھیجیں۔ descriptor کا اختیاری metadatos de caché سیکشن براہ راست اس فائل
کا حوالہ دے سکتا ہے، جس سے control de cambios ریویورز ٹھیک انہی puntos finales پر متفق
رہیں جو cutover کے دوران ras کیے جاتے ہیں۔

ہر DG-3 پیکٹ کو promoción + lista de verificación de reversión بھی درکار ہوتی ہے۔ اسے
`cargo xtask soradns-route-plan` کے ذریعے بنائیں تاکہ control de cambios ریویورز
ہر alias کے لئے verificación previa, corte, اور rollback مراحل کا سراغ لگا سکیں:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` میں hosts canónicos/bonitos, verificación de estado por etapas
Enlaces de GAR, purgas de caché, acciones de reversión, etc.
GAR/encuadernación/transmisión آرٹیفیکٹس کے ساتھ اسے بنڈل کریں تاکہ Ops ایک ہی con guión
قدموں کی مشق اور منظوری دے سکیں۔

`scripts/generate-dns-cutover-plan.mjs` اس descriptor کو تیار کرتا ہے اور
`sorafs-pin-release.sh` سے خودکار طور پر چلتا ہے۔ دستی طور پر دوبارہ بنانے
یا تبدیل کرنے کے لئے:

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

asistente de pin چلانے سے پہلے metadatos opcionales کو variables de entorno کے ذریعے
بھریں:| Variables | Propósito |
|----------|---------|
| `DNS_CHANGE_TICKET` | descriptor میں محفوظ ہونے والا Ticket ID۔ |
| `DNS_CUTOVER_WINDOW` | Ventana de transición ISO8601 (modelo `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | nombre de host de producción + zona autorizada۔ |
| `DNS_OPS_CONTACT` | alias de guardia یا contacto de escalada۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | descriptor میں ریکارڈ ہونے y punto final de purga de caché۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | token de purga رکھنے y env var (predeterminado: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | metadatos de reversión کے لئے سابقہ ​​descriptor de transición کا ruta۔ |

Revisión de cambios de DNS کے ساتھ JSON منسلک کریں تاکہ resúmenes del manifiesto de aprobadores, alias
enlaces, اور sonda کمانڈز کو CI لاگز کریدے بغیر verificar کر سکیں۔ Banderas CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, y `--previous-dns-plan` y anulan فراہم کرتے ہیں
جب helper کو CI کے باہر چلایا جائے۔

## مرحلہ 8 — esqueleto del archivo de zona de resolución بنائیں (اختیاری)جب ventana de transición de producción معلوم ہو تو ریلیز اسکرپٹ Esqueleto de archivo de zona SNS اور
fragmento de resolución خودکار طور پر بنا سکتا ہے۔ مطلوبہ registros DNS y metadatos کو
variables de entorno یا Opciones CLI کے ذریعے پاس کریں؛ descriptor de transición auxiliar
بننے کے فوراً بعد `scripts/sns_zonefile_skeleton.py` چلائے گا۔ کم از کم ایک
A/AAAA/CNAME y un resumen de GAR (carga útil de GAR en BLAKE3-256)
کریں۔ اگر zona/nombre de host معلوم ہوں اور `--dns-zonefile-out` چھوڑ دیا جائے تو
ayudante `artifacts/sns/zonefiles/<zone>/<hostname>.json` پر لکھتا ہے اور
`ops/soradns/static_zones.<hostname>.json` Fragmento de resolución de código کے طور پر بھرتا ہے۔| Variable/bandera | Propósito |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | تیار شدہ esqueleto de archivo de zona کا ruta۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Ruta del fragmento de resolución (اگر چھوڑا جائے تو default `ops/soradns/static_zones.<hostname>.json`)۔ |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | تیار شدہ ریکارڈز پر لاگو TTL (predeterminado: 600 segundos)۔ |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direcciones IPv4 (entorno separado por comas یا indicador CLI repetible) ۔ |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direcciones IPv6۔ |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Objetivo CNAME۔ |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pines SHA-256 SPKI (base64) ۔ |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT (`key=value`) ۔ |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | versión calculada del archivo de zona لیبل کو anular کریں۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Inicio de ventana de transición کے بجائے `effective_at` marca de tiempo (RFC3339) کو forzado کریں۔ |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | metadatos میں ریکارڈ prueba de anulación literal کریں۔ |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | metadatos میں ریکارڈ Anulación de CID کریں۔ |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelación de Guardian (suave, duro, descongelación, monitoreo, emergencia) ۔ |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | congela کے لئے Referencia de billete de tutor/consejo۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | descongelación کے لئے RFC3339 marca de tiempo ۔ || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | اضافی congelar notas (entorno separado por comas یا bandera repetible) ۔ |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | carga útil GAR firmada کا resumen BLAKE3-256 (hexadecimal) ۔ enlaces de puerta de enlace ہونے پر لازم۔ |

Flujo de trabajo de GitHub Actions یہ ویلیوز secretos del repositorio سے پڑھتا ہے تاکہ ہر
pin de producción خودکار طور پر Zonefile آرٹیفیکٹس بنائے۔ درج ذیل secretos/valores
کنفیگر کریں (cadenas میں multivalor فیلڈز کے لئے فہرستیں ہو سکتی ہیں separadas por comas):

| Secreto | Propósito |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | ayudante کو دیا جانے والا nombre de host/zona de producción ۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | descriptor میں محفوظ alias de guardia۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | شائع ہونے والے Registros IPv4/IPv6۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Objetivo CNAME۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | pines base64 SPKI ۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی Entradas TXT۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | esqueleto میں محفوظ congelar metadatos۔ |
| `DOCS_SORAFS_GAR_DIGEST` | carga útil GAR firmada کا resumen BLAKE3 codificado en hexadecimal ۔ |

`.github/workflows/docs-portal-sorafs-pin.yml` چلاتے y `dns_change_ticket` اور
`dns_cutover_window` entradas فراہم کریں تاکہ descriptor/zonefile درست cambiar ventana
metadatos لے۔ انہیں خالی صرف simulacros کے لئے چھوڑیں۔

Invocación de عام (runbook del propietario de SN-7 کے مطابق):

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
```ayudante خودکار طور پر cambiar ticket کو entrada TXT کے طور پر لے جاتا ہے اور corte
inicio de ventana کو `effective_at` marca de tiempo کے طور پر hereda کرتا ہے جب تک اسے
anular نہ کیا جائے۔ مکمل عملی flujo de trabajo کے لئے
`docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### پبلک DNS ڈیلیگیشن نوٹ

esqueleto de archivo de zona صرف زون کے autoritativo ریکارڈز متعین کرتا ہے۔ عام انٹرنیٹ کو
نام سرورز ملنے کے لئے zona principal کی Delegación NS/DS رجسٹرار یا DNS فراہم کنندہ
پر الگ سے سیٹ کرنا ضروری ہے۔
- transición de apex/TLD کے لئے ALIAS/ANAME (específico del proveedor) استعمال کریں یا gateway
  کے IP Anycast کی طرف اشارہ کرنے والے A/AAAA ریکارڈز شائع کریں۔
- سب ڈومینز کے لئے derivado bastante host (`<fqdn>.gw.sora.name`) کی طرف CNAME
  شائع کریں۔
- puerta de enlace del host canónico (`<hash>.gw.sora.id`) ڈومین کے تحت رہتا ہے اور آپ کے
  پبلک زون میں شائع نہیں ہوتا۔

### Puerta de enlace ہیڈر ٹیمپلیٹ

implementar ayudante `portal.gateway.headers.txt` o `portal.gateway.binding.json` بھی
Este es el requisito de enlace de contenido de puerta de enlace DG-3:

- `portal.gateway.headers.txt` Bloque de encabezado HTTP رکھتا ہے (جس میں
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, etc.
  `Sora-Route-Binding` descriptor شامل ہیں) جسے puertas de enlace de borde ہر respuesta کے
  ساتھ grapado کرتے ہیں۔
- `portal.gateway.binding.json` وہی معلومات legible por máquina شکل میں ریکارڈ کرتا
  ہے تاکہ cambiar tickets اور automatización host/enlaces cid کو shell آؤٹ پٹ کریدے
  بغیر diff کر سکیں۔یہ خودکار طور پر `cargo xtask soradns-binding-template` کے ذریعے جنریٹ ہوتے ہیں اور alias, manifest digest, اور gateway
nombre de host کو محفوظ کرتے ہیں جو `sorafs-pin-release.sh` کو فراہم کیے گئے تھے۔
bloque de encabezado دوبارہ بنانے یا personalizar کرنے کے لئے چلائیں:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`, `--permissions-template`, یا `--hsts-template` کے ذریعے predeterminado
plantillas de encabezado کو anular کریں جب کسی مخصوص کو اضافی directivas درکار ہوں؛
انہیں موجودہ `--no-*` interruptores کے ساتھ ملا کر کسی encabezado کو مکمل طور پر ہٹائیں۔

fragmento de encabezado y solicitud de cambio de CDN
canalización de automatización de puerta de enlace میں feed کریں تاکہ اصل promoción de host ریلیز evidencia
سے میل کھائے۔

ayudante de xtask اب canonical راستہ ہے۔ ریلیز اسکرپٹ ayudante de verificación خودکار طور
پر چلاتا ہے تاکہ DG-3 ٹکٹس میں ہمیشہ تازہ evidencia شامل ہو۔ جب بھی آپ enlace JSON
دستی طور پر تبدیل کریں تو اسے دوبارہ چلائیں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

یہ کمانڈ grapado `Sora-Proof` carga útil کو ڈی کوڈ کرتی ہے، `Sora-Route-Binding`
metadatos کو manifiesto CID + nombre de host کے ساتھ میچ کرتی ہے، اور اگر کوئی deriva del encabezado
ہو تو فوراً falla کرتی ہے۔ جب بھی آپ یہ کمانڈ CI کے باہر چلائیں تو کنسول آؤٹ پٹ کو
Implementación de دیگر آرٹیفیکٹس کے ساتھ آرکائیو کریں تاکہ DG-3 ریویورز کے پاس ثبوت ہو
کہ corte de encuadernación سے پہلے validar ہوا تھا۔> **Integración del descriptor DNS:** `portal.dns-cutover.json` o `gateway_binding`
> سیکشن شامل کرتا ہے جو ان آرٹیفیکٹس (rutas, contenido CID, estado de prueba, اور
> plantilla de encabezado literal) کی طرف اشارہ کرتا ہے **اور** Estrofa `route_plan`
> `gateway.route_plan.json` اور plantillas de encabezado principal + reversión کا حوالہ دیتا ہے۔
> ہر Billete de cambio DG-3 میں یہ بلاکس شامل کریں تاکہ ریویورز عین
> `Sora-Name`/`Sora-Proof`/`CSP` ویلیوز کا فرق دیکھ سکیں اور تصدیق کر سکیں کہ
> promoción/reversión de ruta پلانز paquete de evidencia کے ساتھ مطابقت رکھتے ہیں بغیر
> construir archivo کھولے۔

## مرحلہ 9 — monitores de publicación چلائیں

روڈ میپ ٹاسک **DOCS-3c** کے لئے مسلسل evidencia درکار ہے کہ پورٹل، Pruébelo پروکسی،
اور enlaces de puerta de enlace ریلیز کے بعد صحت مند رہیں۔ مراحل 7-8 کے فوراً بعد consolidado
monitor چلائیں اور اسے sondas programadas میں cable کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- Configuración `scripts/monitor-publishing.mjs` فائل لوڈ کرتا ہے (esquema کے لئے
  `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) اور تین cheques
  چلاتا ہے: sondeos de ruta del portal + CSP/validación de políticas de permisos, Pruébelo como proxy
  sondas (اختیاری طور پر `/metrics` endpoint ہٹ کر), اور verificador de enlace de puerta de enlace
  (`cargo xtask soradns-verify-binding`) Verificaciones de alias/manifiesto کے ساتھ
  Sora-Content-CID کی موجودگی + متوقع ویلیو نافذ کرتا ہے۔
- اگر کوئی بھی sonda ناکام ہو تو کمانڈ distinto de cero سے نکلتی ہے تاکہ CI, trabajos cron,
  یا alias de operadores de runbook پروموٹ کرنے سے پہلے ریلیز روک سکیں۔
- `--json-out` Resumen de carga útil JSON لکھتا ہے جس میں فی estado del objetivo ہے؛
  `--evidence-dir` `summary.json`, `portal.json`, `tryit.json`, `binding.json`, Otros
  `checksums.sha256` جاری کرتا ہے تاکہ گورننس ریویورز بغیر monitores دوبارہ چلائے
  نتائج diff کر سکیں۔ اس ڈائریکٹری کو `artifacts/sorafs/<tag>/monitoring/` میں
  Paquete Sigstore اور Descriptor de transición de DNS کے ساتھ آرکائیو کریں۔
- monitorizar la exportación Grafana (`dashboards/grafana/docs_portal.json`), اور
  ID de perforación de Alertmanager کو ریلیز ٹکٹ میں شامل کریں تاکہ DOCS-3c SLO بعد میں
  آڈٹ ہو سکے۔ Guía de monitorización de publicaciones dedicada یہاں ہے:
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

El portal sondea HTTPS کا تقاضا کرتے ہیں اور `http://` بیس URL کو مسترد کرتے ہیں جب
Configuración del monitor میں `allowInsecureHttp` سیٹ نہ ہو؛ objetivos de producción/puesta en escena
کو TLS پر رکھیں اور یہ anular صرف لوکل vistas previas کے لئے فعال کریں۔پورٹل کے لائیو ہونے کے بعد `npm run monitor:publishing` کو Buildkite/cron میں
automatizar کریں۔ یہی کمانڈ، جب URL de producción پر puntiagudas ہو، وہ صحت مند چیکس فراہم
کرتی ہے جن پر SRE/Docs ریلیز کے درمیان انحصار کرتے ہیں۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` Plantilla 2-6 کو encapsular کرتا ہے۔ یہ:

1. `build/` Tarball determinista میں آرکائیو کرتا ہے،
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   اور `proof verify` چلاتا ہے،
3. Credenciales Torii موجود ہوں تو اختیاری طور پر `manifest submit` (alias vinculante سمیت)
   چلاتا ہے، اور
4. `artifacts/sorafs/portal.pin.report.json`, اختیاری `portal.pin.proposal.json`,
  Descriptor de transferencia de DNS (envíos کے بعد), un paquete de enlace de puerta de enlace
  (`portal.gateway.binding.json` + bloque de encabezado de texto) لکھتا ہے تاکہ gobernanza,
  networking, اور ops ٹیمیں CI لاگز کھنگالے بغیر paquete de evidencia کا فرق دیکھ سکیں۔

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, اور (اختیاری)
`PIN_ALIAS_PROOF_PATH` Negro کریں۔ ensayos en seco کے لئے `--skip-submit` استعمال کریں؛
Flujo de trabajo de GitHub نیچے بیان کردہ `perform_submit` input سے اسے ٹوگل کرتا ہے۔

## مرحلہ 8 - Especificaciones de OpenAPI اور Paquetes SBOM شائع کریں

DOCS-7 تقاضہ کرتا ہے کہ پورٹل build, OpenAPI spec, اور SBOM آرٹیفیکٹس ایک ہی
tubería determinista سے گزریں۔ Otros ayudantes تینوں کو کور کرتے ہیں:

1. **especificaciones دوبارہ بنائیں اور سائن کریں۔**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```جب بھی تاریخی instantánea محفوظ کرنا ہو تو `--version=<label>` کے ذریعے ریلیز
   لیبل فراہم کریں (مثلاً `2025-q3`)۔ instantánea de ayuda کو
   `static/openapi/versions/<label>/torii.json` میں لکھتا ہے، اسے `versions/current`
   میں mirror کرتا ہے، اور metadatos (SHA-256, estado del manifiesto, marca de tiempo actualizada)
   `static/openapi/versions.json` میں ریکارڈ کرتا ہے۔ ڈیولپر پورٹل یہ índice پڑھتا
   ہے تاکہ Swagger/RapiDoc پینلز selector de versiones دکھا سکیں اور متعلقہ resumen/firma
   información en línea پیش کر سکیں۔ `--version` چھوڑ دینے سے پچھلے ریلیز لیبل برقرار رہتے
   ہیں اور صرف `current` + `latest` punteros تازہ ہوتے ہیں۔

   manifiesto SHA-256/BLAKE3 resúmenes محفوظ کرتا ہے تاکہ gateway `/reference/torii-swagger`
   کے لئے `Sora-Proof` encabezados grapados کر سکے۔

2. **SBOM CycloneDX خارج کریں۔** ریلیز pipeline پہلے ہی SBOM basados en syft کی توقع
   رکھتی ہے، جیسا کہ `docs/source/sorafs_release_pipeline_plan.md` میں درج ہے۔
   آؤٹ پٹ کو construir artefactos کے ساتھ رکھیں:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **ہر carga útil کو CAR میں پیک کریں۔**

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

   مین سائٹ جیسے ہی `manifest build` / `manifest sign` مراحل پر عمل کریں، اور
   ہر اثاثے کے لئے alias ٹیون کریں (مثلاً spec کے لئے `docs-openapi.sora` اور
   سائن شدہ SBOM بنڈل کے لئے `docs-sbom.sora`)۔ الگ alias رکھنے سے Pruebas de SoraDNS,
   GARs, اور rollback ٹکٹس مخصوص carga útil تک محدود رہتے ہیں۔4. **جمع کریں اور bind کریں۔** وہی autoridad + paquete Sigstore دوبارہ استعمال کریں،
   مگر ریلیز lista de verificación میں alias tupla ریکارڈ کریں تاکہ آڈیٹرز دیکھ سکیں کہ کون سا
   Sora نام کس resumen manifiesto سے منسلک ہے۔

Manifestaciones de especificación/SBOM کو پورٹل build کے ساتھ آرکائیو کرنا یقینی بناتا ہے کہ ہر
ریلیز ٹکٹ میں مکمل آرٹیفیکٹ سیٹ موجود ہو بغیر packer دوبارہ چلائے۔

### آٹومیشن ہیلپر (CI/script de paquete)

`./ci/package_docs_portal_sorafs.sh` Código 1-8 کو codificación کرتا ہے تاکہ روڈ میپ
آئٹم **DOCS-7** کو ایک کمانڈ میں چلایا جا سکے۔ ہیلپر:

- مطلوبہ پورٹل prep چلاتا ہے (`npm ci`, OpenAPI/norito sync, pruebas de widgets) ؛
- پورٹل، OpenAPI, اور SBOM CARs + pares de manifiesto کو `sorafs_cli` کے ذریعے بناتا ہے؛
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور Sigstore firma
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`) چلاتا ہے؛
- تمام آرٹیفیکٹس کو `artifacts/devportal/sorafs/<timestamp>/` کے تحت چھوڑتا ہے اور
  `package_summary.json` Herramientas de lanzamiento/CI para ingerir archivos اور
- `artifacts/devportal/sorafs/latest` کو ریفریش کر کے تازہ ترین رن کی طرف اشارہ کرتا ہے۔

مثال (Sigstore + PoR کے ساتھ مکمل tubería):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

قابلِ توجہ banderas:- `--out <dir>` – anulación de raíz de artefacto کریں (marca de tiempo predeterminada فولڈرز رکھتا ہے)۔
- `--skip-build` – موجودہ `docs/portal/build` دوبارہ استعمال کریں (جب CI آف لائن
  espejos کی وجہ سے reconstruir نہ کر سکے)۔
- `--skip-sync-openapi` – `npm run sync-openapi` چھوڑ دیں جب `cargo xtask openapi`
  crates.io تک نہ پہنچ سکے۔
- `--skip-sbom` – جب `syft` بائنری انسٹال نہ ہو تو اسے نہ چلائیں (اسکرپٹ advertencia دیتا ہے)۔
- `--proof` – ہر CAR/manifiesto جوڑی کے لئے `sorafs_cli proof verify` چلائیں۔ archivos múltiples
  cargas útiles CLI میں fragment-plan سپورٹ مانگتے ہیں، اس لئے اگر `plan chunk count`
  errores آئیں تو یہ فلیگ بند رکھیں اور upstream gate آنے پر دستی تصدیق کریں۔
- `--sign` – `sorafs_cli manifest sign` چلائیں۔ Teléfono `SIGSTORE_ID_TOKEN` Negro
  (یا `--sigstore-token-env`) یا CLI کو `--sigstore-provider/--sigstore-audience`
  کے ذریعے اسے buscar کرنے دیں۔

artefactos de producción کے لئے `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
یہ اب پورٹل، OpenAPI، اور SBOM payloads پیک کرتا ہے، ہر manifest پر سائن کرتا ہے،
اور `portal.additional_assets.json` میں اضافی اثاثوں کا metadatos ریکارڈ کرتا ہے۔
ayudante y perillas سمجھتا ہے جو empaquetador CI استعمال کرتا ہے، ساتھ ہی نئے
`--openapi-*`, `--portal-sbom-*`, y `--openapi-sbom-*` cambian de color
ہر اثاثے کے لئے alias tuplas مقرر کر سکیں، `--openapi-sbom-source` کے ذریعے SBOM
ماخذ anular کر سکیں، مخصوص cargas útiles چھوڑ کیں (`--skip-openapi`/`--skip-sbom`),
اور no predeterminado `syft` بائنری کی طرف `--syft-bin` سے اشارہ کر سکیں۔یہ اسکرپٹ ہر کمانڈ دکھاتا ہے جو وہ چلاتا ہے؛ iniciar sesión `package_summary.json` کے ساتھ
ریلیز ٹکٹ میں شامل کریں تاکہ ریویورز CAR digests, metadatos del plan, اور Sigstore paquete
hashes کا فرق shell ad-hoc آؤٹ پٹ دیکھے بغیر نکال سکیں۔

## مرحلہ 9 — Gateway + SoraDNS کی تصدیق

cutover کا اعلان کرنے سے پہلے ثابت کریں کہ نیا alias SoraDNS کے ذریعے resolve ہو
رہا ہے اور pasarelas تازہ pruebas grapadas کر رہے ہیں:

1. **puerta de sonda چلائیں۔** `ci/check_sorafs_gateway_probe.sh` ڈیمو accesorios کے خلاف
   `cargo xtask sorafs-gateway-probe` چلاتا ہے جو
   `fixtures/sorafs_gateway/probe_demo/` میں موجود ہیں۔ حقیقی implementaciones کے لئے
   sonda کو ہدف nombre de host پر چلائیں:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   sonda `Sora-Name`, `Sora-Proof`, y `Sora-Proof-Status` کو
   `docs/source/sorafs_alias_policy.md` کے مطابق decodificar کرتا ہے اور اگر resumen manifiesto,
   TTL, enlaces GAR y deriva ہو تو ناکام ہو جاتا ہے۔

   Para controles puntuales ligeros (por ejemplo, cuando sólo el paquete de encuadernación
   cambiado), ejecute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   El asistente valida el paquete de enlaces capturado y es útil para su lanzamiento.
   boletos que solo necesitan confirmación vinculante en lugar de un simulacro de sonda completa.2. **evidencia de simulacro جمع کریں۔** آپریٹر simulacros یا PagerDuty simulacros کے لئے sonda کو
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- …`
   کے ساتھ envoltura کریں۔ encabezados/registros de contenedor
   `artifacts/sorafs_gateway_probe/<stamp>/` میں رکھتا ہے، `ops/drill-log.md` اپ ڈیٹ
   کرتا ہے، اور (اختیاری) ganchos de reversión یا cargas útiles de PagerDuty بھی چلاتا ہے۔ SoraDNS
   راستے کی تصدیق کے لئے `--host docs.sora` دیں تاکہ Código rígido IP نہ ہو۔

3. **Enlaces DNS کی تصدیق کریں۔** جب گورننس prueba de alias شائع کرے تو sonda میں
   حوالہ دیا گیا GAR فائل (`--gar`) ریکارڈ کریں اور ریلیز evidencia کے ساتھ منسلک کریں۔
   Resolver funciona con el `tools/soradns-resolver` y utiliza el software de resolución
   کہ entradas en caché نئے manifiesto کو honor کریں۔ JSON منسلک کرنے سے پہلے چلائیں
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Mapeo de host determinista, metadatos de manifiesto, etiquetas de telemetría fuera de línea
   validar ہو جائیں۔ ayudante firmado GAR کے ساتھ `--json-out` resumen بھی نکال سکتا ہے
   تاکہ revisores بغیر binario کھولے قابلِ تصدیق evidencia حاصل کریں۔
  نیا GAR بناتے وقت ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` پر صرف تب جائیں جب manifiesto فائل دستیاب نہ ہو)۔ ayudante اب
  CID **اور** BLAKE3 resumen del manifiesto JSON سے براہ راست نکالتا ہے، espacios en blanco
  recortar کرتا ہے، دہرے `--telemetry-label` banderas ختم کرتا ہے، etiquetas ordenar کرتا ہے، اور
  JSON لکھنے سے پہلے CSP/HSTS/plantillas de política de permisos predeterminadas نکالتا ہے تاکہ
  carga útil determinista رہے، چاہے آپریٹرز etiquetas مختلف shells سے اکٹھے کریں۔4. **alias métricas پر نظر رکھیں۔** `torii_sorafs_alias_cache_refresh_duration_ms`
   Sonda `torii_sorafs_gateway_refusals_total{profile="docs"}` کو کے دوران
   اسکرین پر رکھیں؛ Serie دونوں `dashboards/grafana/docs_portal.json` میں چارٹ ہیں۔

## مرحلہ 10 — Monitoreo y agrupación de evidencia

- **Paneles ۔** ہر ریلیز کے لئے `dashboards/grafana/docs_portal.json` (portal SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (latencia de puerta de enlace + estado de prueba) ,
  اور `dashboards/grafana/sorafs_fetch_observability.json` (salud del orquestador)
  exportar کریں۔ Exportaciones JSON
  Consultas Prometheus دوبارہ چلا سکیں۔
- **Archivos de sonda ۔** `artifacts/sorafs_gateway_probe/<stamp>/` کو git-annex یا
  اپنے cubo de pruebas میں رکھیں۔ resumen de la sonda, encabezados, script de telemetría,
  پکڑا گیا Carga útil de PagerDuty شامل کریں۔
- **Paquete de lanzamiento۔** پورٹل/SBOM/OpenAPI Resúmenes de CAR, paquetes de manifiestos, Sigstore
  firmas, `portal.pin.report.json`, registros de sonda Try-It, e informes de verificación de enlaces
  ایک con marca de tiempo فولڈر (مثال `artifacts/sorafs/devportal/20260212T1103Z/`) میں رکھیں۔
- **Registro de perforación۔** جب sondas کسی taladro کا حصہ ہوں تو
  `scripts/telemetry/run_sorafs_gateway_probe.sh` y `ops/drill-log.md` también agregar
  کرنے دیں تاکہ وہی evidencia del requisito del caos SNNet-5 پوری کرے۔
- **Enlaces de boletos۔** cambiar boleto میں ID del panel Grafana یا exportaciones PNG adjuntas کا
  حوالہ دیں، ساتھ ہی ruta del informe de sonda بھی شامل کریں، تاکہ acceso al shell de revisores
  کے بغیر SLO verificación cruzada کر سکیں۔

## مرحلہ 11 - simulacro de búsqueda de múltiples fuentes y evidencia del marcadorSoraFS پر اشاعت اب evidencia de búsqueda de fuentes múltiples (DOCS-7/SF-6) کا تقاضہ کرتی ہے،
اور یہ اوپر والے DNS/pruebas de puerta de enlace کے ساتھ ہونا چاہیے۔ pin de manifiesto کرنے کے بعد:

1. **manifiesto en vivo کے خلاف `sorafs_fetch` چلائیں۔** مراحل 2-3 کے plan/manifiesto
   آرٹیفیکٹس اور ہر proveedor کے جاری کردہ credenciales de puerta de enlace استعمال کریں۔ ہر
   آؤٹ پٹ محفوظ کریں تاکہ آڈیٹرز orquestador کے seguimiento de decisiones کو repetición کر سکیں:

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

   - manifiesto میں حوالہ دی گئی anuncios del proveedor پہلے حاصل کریں (مثلاً
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور انہیں `--provider-advert name=path` کے ذریعے پاس کریں تاکہ marcador
     ventanas de capacidad deterministas انداز میں evaluar کر سکے۔
     `--allow-implicit-provider-metadata` **صرف** Los accesorios de CI میں repiten کرتے وقت
     استعمال کریں؛ taladros de producción میں pin کے ساتھ آنے والے anuncios firmados ہی دیں۔
   - جب manifiesto اضافی regiones کی طرف اشارہ کرے تو متعلقہ tuplas de proveedores کے ساتھ
     کمانڈ دہرائیں تاکہ ہر caché/alias کی artefacto de búsqueda coincidente ہو۔

2. **آؤٹ پٹس آرکائیو کریں۔** `scoreboard.json`, `providers.ndjson`, `fetch.json`, اور
   `chunk_receipts.ndjson` کو ریلیز evidencia فولڈر کے اندر رکھیں۔ یہ فائلیں compañero
   ponderación, reintento de presupuesto, latencia EWMA, recibos por fragmento محفوظ کرتی ہیں
   جنہیں گورننس پیکٹ SF-7 کے لئے برقرار رکھے۔3. **ٹیلیمیٹری اپ ڈیٹ کریں۔** buscar salidas کو **SoraFS Obtener observabilidad**
   ڈیش بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) میں امپورٹ کریں،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` اور paneles de gama de proveedores پر
   anomalías دیکھیں۔ Grafana instantáneas del panel کو ریلیز ٹکٹ میں ruta del marcador کے ساتھ
   لنک کریں۔

4. **reglas de alerta کا humo کریں۔** `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   چلا کر ریلیز بند کرنے سے پہلے Prometheus validar paquete de alertas کریں۔ herramienta de promoción
   آؤٹ پٹ کو ٹکٹ کے ساتھ منسلک کریں تاکہ DOCS-7 ریویورز تصدیق کر سکیں کہ puesto اور
   alertas de proveedor lento مسلح رہیں۔

5. **CI میں wire کریں۔** پورٹل pin flujo de trabajo میں `sorafs_fetch` مرحلہ `perform_fetch_probe`
   entrada کے پیچھے رکھا گیا ہے؛ puesta en escena/producción رنز میں اسے فعال کریں تاکہ buscar
   paquete de manifiesto de evidencia کے ساتھ خودکار طور پر تیار ہو۔ لوکل ejercicios وہی اسکرپٹ
   استعمال کر سکتے ہیں جب gateway tokens export ہوں اور `PIN_FETCH_PROVIDERS` کو
   lista de proveedores separados por comas سے سیٹ کیا جائے۔

## پروموشن، observabilidad, اور reversión1. **پروموشن:** puesta en escena اور producción کے لئے علیحدہ alias رکھیں۔ پروموشن کے لئے
   اسی manifiesto/paquete کے ساتھ `manifest submit` دوبارہ چلائیں اور
   `--alias-namespace/--alias-name` کو alias de producción پر سوئچ کریں۔ اس سے QA منظور
   شدہ pin de preparación کے بعد دوبارہ construir یا volver a firmar کی ضرورت نہیں رہتی۔
2. **Monitoreo:** registro pin ڈیش بورڈ
   (`docs/source/grafana_sorafs_pin_registry.json`) اور پورٹل مخصوص sondas
   (`docs/portal/docs/devportal/observability.md` دیکھیں) امپورٹ کریں۔ deriva de la suma de comprobación,
   sondas fallidas, picos de reintento de prueba y alerta کریں۔
3. **Revertir:** واپس جانے کے لئے پچھلا manifiesto دوبارہ enviar کریں (یا موجودہ alias
   retirarse کریں) `sorafs_cli manifest submit --alias ... --retire` کے ذریعے۔ ہمیشہ
   último paquete en buen estado اور CAR resumen محفوظ رکھیں تاکہ CI لاگز rotar ہو جائیں تو
   pruebas de reversión دوبارہ بن سکیں۔

## Plantilla de flujo de trabajo de CI

کم از کم، آپ کی tubería کو یہ کرنا چاہیے:

1. Build + lint (`npm ci`, `npm run build`, generación de suma de comprobación).
2. Paquete (`car pack`) اور manifiesta computar کریں۔
3. OIDC con ámbito de trabajo ٹوکن کے ساتھ signo کریں (`manifest sign`).
4. Auditoría de artefactos کے لئے اپ لوڈ کریں (CAR, manifiesto, paquete, plan, resúmenes).
5. pin de registro میں enviar کریں:
   - Solicitudes de extracción → `docs-preview.sora`.
   - Etiquetas/ramas protegidas → promoción de alias de producción.
6. salida سے پہلے sondas + puertas de verificación de prueba چلائیں۔

Lanzamientos manuales `.github/workflows/docs-portal-sorafs-pin.yml` کے لئے یہ تمام
مراحل جوڑتا ہے۔ flujo de trabajo:- پورٹل construir/probar کرتا ہے،
- `scripts/sorafs-pin-release.sh` کے ذریعے build پیکج کرتا ہے،
- GitHub OIDC استعمال کر کے paquete de manifiesto سائن/verify کرتا ہے،
- CAR/manifiesto/paquete/plan/resúmenes de prueba کو artefactos کے طور پر اپ لوڈ کرتا ہے، اور
- (اختیاری) secretos موجود ہوں تو manifiesto + alias vinculante enviar کرتا ہے۔

trabajo چلانے سے پہلے درج ذیل secretos/variables del repositorio کنفیگر کریں:

| Nombre | Propósito |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii host y `/v1/sorafs/pin/register` son hosts ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | envíos کے ساتھ ریکارڈ ہونے والا identificador de época۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | presentación de manifiesto کے لئے autoridad de firma۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tupla جو `perform_submit` true ہونے پر manifest سے bind ہوتا ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Paquete de prueba de alias codificado en Base64 (اختیاری؛ enlace de alias چھوڑنے کے لئے omitir کریں)۔ |
| `DOCS_ANALYTICS_*` | موجودہ análisis/puntos finales de sonda جو دوسرے flujos de trabajo میں reutilización ہوتے ہیں۔ |

Interfaz de usuario de acciones y activador de flujo de trabajo:

1. `alias_label` فراہم کریں (مثلاً `docs.sora.link`), opcional `proposal_alias`,
   Anulación opcional `release_tag` ۔
2. artefactos بنانے کے لئے `perform_submit` کو unchecked چھوڑیں (Torii کو touch کیے بغیر)
   یا اسے habilitar کریں تاکہ alias configurado پر براہ راست publicar ہو جائے۔`docs/source/sorafs_ci_templates.md` اب بھی اس repo کے باہر کے پروجیکٹس کے لئے
asistentes genéricos de CI

## Lista de verificación

- [] `npm run build`, `npm run test:*`, y `npm run check:links` گرین ہیں۔
- [] `build/checksums.sha256` اور `build/release.json` artefactos میں محفوظ ہیں۔
- [] COCHE, plan, manifiesto, resumen `artifacts/` کے تحت بنے ہیں۔
- [] Paquete Sigstore + firma separada لاگز کے ساتھ محفوظ ہیں۔
- [] `portal.manifest.submit.summary.json` y `portal.manifest.submit.response.json`
      presentaciones کے وقت محفوظ ہوئے۔
- [ ] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
      CAR/artefactos manifiestos کے ساتھ آرکائیو ہوئے۔
- [] `proof verify` اور `manifest verify-signature` لاگز آرکائیو ہوئے۔
- [] Cuadros de mando Grafana اپ ڈیٹ ہیں + sondas Try-It کامیاب ہیں۔
- [] notas de reversión (ID de manifiesto anterior + resumen de alias) ریلیز ٹکٹ کے ساتھ منسلک ہیں۔