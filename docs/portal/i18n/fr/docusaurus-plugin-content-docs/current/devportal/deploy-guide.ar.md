---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## نظرة عامة

**DOCS-7** (numéro SoraFS) et **DOCS-8** (broche de CI/CD) pour CI/CD لبوابة المطورين. Vous pouvez utiliser build/lint pour SoraFS, pour créer un lien vers Sigstore. التحقق، وتمارين التراجع بحيث تكون كل معاينة واصدار قابلة لاعادة الانتاج وقابلة للتدقيق.

Il s'agit d'un point de terminaison `sorafs_cli` (avec `--features cli`) et d'un point de terminaison Torii. pin-registry, et correspond à OIDC pour Sigstore. خزّن الاسرار طويلة العمر (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, رموز Torii) pour CI؛ Il y a des exportations vers le shell.

## المتطلبات المسبقة- Nœud 18.18 et `npm` et `pnpm`.
- `sorafs_cli` ou `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان Torii يكشف `/v2/sorafs/*` مع حساب/مفتاح خاص بسلطة يمكنها ارسال المانيفست والاسماء المستعارة.
- Utilisez OIDC (Actions GitHub, GitLab, identité de charge de travail, ici) pour `SIGSTORE_ID_TOKEN`.
- Version : `examples/sorafs_cli_quickstart.sh` pour les workflows et `docs/source/sorafs_ci_templates.md` pour les workflows sur GitHub/GitLab.
- اضبط متغيرات OAuth الخاصة بـ Essayez-le (`DOCS_OAUTH_*`) et
  [liste de contrôle pour le renforcement de la sécurité](./security-hardening.md) قبل ترقية build خارج المختبر. بناء البوابة يفشل الان عند غياب هذه المتغيرات او عند خروج مفاتيح TTL/polling عن النوافذ المفروضة؛ استخدم `DOCS_OAUTH_ALLOW_INSECURE=1` فقط لمعاينات محلية مؤقتة. ارفق ادلة اختبار الاختراق مع تذكرة الاصدار.

## الخطوة 0 - التقاط حزمة وكيل Essayez-le

قبل ترقية المعاينة الى Netlify او البوابة، ثبّت مصادر وكيل Try it وبصمة المانيفست OpenAPI الموقّع في حزمة حتمية:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` est un proxy/probe/rollback et est également OpenAPI et `release.json` ou `checksums.sha256`. Il s'agit d'une passerelle Netlify/SoraFS pour votre projet de connexion Internet. الدقيقة وتلميحات Torii بدون اعادة بناء. Vous avez besoin de jetons de porteur pour le faire (`allow_client_auth`) pour obtenir des jetons de porteur الاطلاق وقواعد CSP.

## الخطوة 1 - بناء وفحص lint للبوابة

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```Pour `npm run build` et `scripts/write-checksums.mjs`:

- `build/checksums.sha256` - Utilisez SHA256 pour `sha256sum -c`.
- `build/release.json` - Prise en main (`tag`, `generated_at`, `source`) pour CAR/Métro.

احفظ كلا الملفين مع ملخص CAR حتى يتمكن المراجعون من مقارنة نواتج المعاينة بدون اعادة بناء.

## الخطوة 2 - تغليف الاصول الثابتة

L'emballeur CAR est pour Docusaurus. Il s'agit de la référence `artifacts/devportal/`.

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

JSON utilise des morceaux, des résumés et des preuves de preuve `manifest build` et CI.

## الخطوة 2b - تغليف مرافقات OpenAPI et SBOM

Le document DOCS-7 est disponible en version OpenAPI et SBOM en version PDF رؤوس `Sora-Proof`/`Sora-Content-CID` pour l'artefact. يقوم مساعد الاصدار (`scripts/sorafs-pin-release.sh`) pour بتغليف مجلد OpenAPI (`static/openapi/`) et SBOM الناتجة عبر `syft` pour CARs remplace `openapi.*`/`*-sbom.*` et `artifacts/sorafs/portal.additional_assets.json`. عند اتباع المسار اليدوي، اعد الخطوات 2-4 لكل payload مع بادئاتها ووسوم metadata الخاصة بها (مثلا `--car-out "$OUT"/openapi.car` مع `--metadata alias_label=docs.sora.link/openapi`). Vérifiez le manifeste/alias de Torii (OpenAPI, SBOM de SBOM OpenAPI) pour le DNS. البوابة من تقديم preuves مدبسة لكل artefact منشور.

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
```Utilisez la stratégie pin-policy pour vous connecter à votre compte (avec `--pin-storage-class hot`). JSON est utilisé pour créer des liens.

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

Il s'agit d'un bundle contenant des morceaux de morceaux et BLAKE3 pour le jeton OIDC pour JWT. احتفظ بالـ bundle والتوقيع المنفصل؛ يمكن لعمليات الترقية في الانتاج اعادة استخدام نفس الاصول بدون اعادة توقيع. يمكن للعمليات المحلية استبدال اعلام المزود بـ `--identity-token-env` (او تعين `SIGSTORE_ID_TOKEN` في البيئة) عندما يصدر مساعد OIDC خارجي التوكن.

## الخطوة 5 - Registre des broches الارسال الى

Ajouter des morceaux (et des morceaux) à Torii. اطلب دائما résumé كي تبقى نتيجة التسجيل/الاسم المستعار قابلة للتدقيق.

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

عند اطلاق معاينة او كاناري (`docs-preview.sora`), اعد الارسال باستخدام alias مختلف حتى يتمكن QA من التحقق قبل ترقية الانتاج.

Les fichiers correspondants sont : `--alias-namespace` et `--alias-name` et `--alias-proof`. Il s'agit d'une preuve de bundle (base64 et Norito octets) خزنه ضمن اسرار CI وقدمه كملف قبل استدعاء `manifest submit`. L'alias est un alias qui est un DNS.

## الخطوة 5b - توليد مقترح حوكمة

يجب ان يرافق كل مانيفست مقترح جاهز للبرلمان بحيث يستطيع اي مواطن من Sora تقديم التغيير دون امتلاك بيانات اعتماد مميزة. بعد خطوات soumettre/signer نفّذ :

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```يلتقط `portal.pin.proposal.json` التعليمة القياسية `RegisterPinManifest` وبصمة الـ chunks et alias. Il s'agit d'une charge utile importante pour la charge utile. هذا الامر لا يلمس مفتاح Torii, لذلك يمكن لاي مواطن ان ينشئ المقترح محليا.

## الخطوة 6 - التحقق من preuves والقياسات

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

- راقب `torii_sorafs_gateway_refusals_total` et `torii_sorafs_replication_sla_total{outcome="missed"}` pour la lecture.
- شغّل `npm run probe:portal` لاختبار وكيل Try-It et على المحتوى المثبت.
- اجمع ادلة المراقبة المذكورة في
  [Publication et surveillance](./publishing-monitoring.md) لتلبية بوابة الملاحظة DOCS-3c. `bindings` (OpenAPI, SBOM ou SBOM OpenAPI) `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` est également compatible avec `hostname`. JSON est compatible avec les applications JSON (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) تحت مجلد الاصدار:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الخطوة 6a - تخطيط شهادات البوابة

Le SAN/challenge TLS est également compatible avec GAR et est compatible avec DNS. Le DG-3 contient un caractère générique et SAN pour Pretty-Host et DNS-01 pour :

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```Pour utiliser JSON, vous devez utiliser la fonction JSON (il s'agit d'une application de type SAN). `torii.sorafs_gateway.acme` ويتأكد مراجعو GAR من الخرائط canonique/jolie دون اعادة الحساب. اضف `--name` لكل لاحقة اضافية يتم ترقيتها في الاصدار نفسه.

## الخطوة 6b - اشتقاق خرائط المضيفين القانونية

Il s'agit d'un pseudonyme de GAR. `cargo xtask soradns-hosts` بتهشئة كل `--name` الى التسمية القانونية (`<base32>.gw.sora.id`) et joker المطلوب (`*.gw.sora.id`) ويشتق المضيف الجميل (`<alias>.gw.sora.name`). احفظ الناتج ضمن artefacts الخريطة مع ارسال GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilisez `--verify-host-patterns <file>` pour créer un lien vers GAR et JSON. يقبل المساعد عدة ملفات لللتحقق، مما يسهل تدقيق قالب GAR et `portal.gateway.binding.json` dans la version actuelle :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

JSON est également compatible avec la connexion DNS/البوابة حتى يستطيع المدققون تأكيد المضيفين القانونية/الجميلة دون اعادة تشغيل السكربتات. اعد تشغيل الامر عند اضافة alias جديد حتى ترث تحديثات GAR نفس الادلة.

## الخطوة 7 - Comment activer le basculement DNS

تتطلب عمليات القطع في الانتاج حزمة تغيير قابلة للتدقيق. بعد نجاح الارسال (ربط الاسم المستعار)، يصدر المساعد
`artifacts/sorafs/portal.dns-cutover.json`, et:- بيانات ربط الاسم المستعار (espace de noms/nom/preuve, بصمة المانيفست، عنوان Torii, epoch الارسال، السلطة).
- Étiquette (tag, alias label, مسارات المانيفست/CAR, خطة الـ chunks, Sigstore bundle).
- مؤشرات التحقق (امر sonde, alias + point de terminaison Torii).
- حقول تغيير اختيارية (معرف التذكرة، نافذة cutover, جهة الاتصال التشغيلية، مضيف/منطقة الانتاج).
- بيانات ترقية المسار المشتقة من رأس `Sora-Route-Binding` (المضيف القانوني/CID, مسارات الرأس + الربط، اوامر التحقق) لضمان ان ترقية GAR وتمارين التراجع تشير لنفس الادلة.
- Plan d'itinéraire (`gateway.route_plan.json`) pour le DG-3 et les peluches du CI خطط الترقية/التراجع.
- بيانات اختيارية لالغاء الكاش (نقطة purge, متغير auth, حمولة JSON, ومثال `curl`).
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

ارفق `portal.cache_plan.json` DG-3 ليحصل المشغلون على مسارات/مضيفين حتميين (ومرشحات auth) عند ارسال طلبات `PURGE`. يمكن للقسم الاختياري في الوصف ان يشير الى هذا الملف مباشرة.

تحتاج حزمة DG-3 ايضا الى قائمة فحص للترقية والتراجع. ولّدها عبر `cargo xtask soradns-route-plan` :

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```يسجل `gateway.route_plan.json` المضيفين القانونيين/الجميلين، تذكيرات فحوص الصحة المرحلية، تحديثات الربط GAR، purge للكاش، وخطوات التراجع. ارفقه مع ملفات GAR/binding/cutover قبل ارسال التذكرة حتى يتمكن فريق Ops من التدريب على الخطوات نفسها.

Le `scripts/generate-dns-cutover-plan.mjs` est remplacé par le `sorafs-pin-release.sh`. للتوليد اليدوي:

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
| `DNS_CUTOVER_WINDOW` | Passage en mode ISO8601 (pour `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | مضيف الانتاج + المنطقة المعتمدة. |
| `DNS_OPS_CONTACT` | جهة الاتصال للطوارئ. |
| `DNS_CACHE_PURGE_ENDPOINT` | نقطة purge المخزنة في الوصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Utilisez la purge de jeton (`CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | مسار الوصف السابق لبيانات التراجع. |

JSON utilise la fonction DNS pour créer une sonde de type JSON بدون فحص سجلات CI. توفر اعلام CLI مثل `--dns-change-ticket` و`--dns-cutover-window` و`--dns-hostname` و`--dns-zone` و`--ops-contact` و`--cache-purge-endpoint` و`--cache-purge-auth-env` و`--previous-dns-plan` نفس امكانية التخصيص خارج CI.

## الخطوة 8 - اصدار هيكل zonefile للمحلل (اختياري)Il s'agit d'un fichier de zone pour SNS et d'un extrait de code. مرر سجلات DNS et عبر متغيرات البيئة او خيارات CLI؛ سيستدعي المساعد `scripts/sns_zonefile_skeleton.py` مباشرة بعد توليد الوصف. Il s'agit de la version A/AAAA/CNAME de GAR (BLAKE3-256 pour la version موقعة). اذا كانت المنطقة/المضيف معروفة وتم حذف `--dns-zonefile-out`, سيكتب المساعد في `artifacts/sns/zonefiles/<zone>/<hostname>.json` ويولد `ops/soradns/static_zones.<hostname>.json` Voici un extrait de résolveur.| المتغير / الخيار | الغرض |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Utilisez le squelette du fichier de zone. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Il s'agit d'un extrait de résolveur (افتراضي `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL pour les frais de port (prix : 600 dollars). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Il s'agit d'IPv4 (c'est-à-dire que vous êtes connecté à Internet et que vous êtes connecté à Internet). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Il s'agit d'IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | C'est CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Utilisez SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Téléchargez TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | تجاوز label النسخة المحسوب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | L'horodatage `effective_at` (RFC3339) est disponible pour vous. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | تجاوز preuve المسجل في البيانات. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | تجاوز CID المسجل في البيانات. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | حالة تجميد الحارس (doux, dur, décongélation, surveillance, urgence). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | مرجع تذكرة التجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | timestamp RFC3339 pour la décongélation. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | ملاحظات تجميد اضافية (قائمة مفصولة بفواصل). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Utilisez BLAKE3-256 (hex) pour la lecture. مطلوبة عند وجود liaisons للبوابة. |Utilisez GitHub Actions pour créer une broche pour les artefacts. اضبط الاسرار التالية (يمكن ان تحتوي القيم على قوائم مفصولة بفواصل):

| Secrets | الغرض |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | مضيف/منطقة الانتاج للمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | جهة الاتصال المخزنة في الوصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Utilisez IPv4/IPv6 pour. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | C'est CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Utilisez SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | سجلات TXT اضافية. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | بيانات التجميد في الهيكل. |
| `DOCS_SORAFS_GAR_DIGEST` | بصمة BLAKE3 بالصيغة السداسية للحمولة الموقعة. |

Utilisez `.github/workflows/docs-portal-sorafs-pin.yml` et `dns_change_ticket` et `dns_cutover_window` pour utiliser/zonefile. Il s'agit d'un fonctionnement à sec.

Nom du propriétaire (runbook du propriétaire du SN-7) :

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

يسحب المساعد تذكرة التغيير تلقائيا كمدخل TXT ويورث بداية نافذة القطع الى `effective_at` ما لم يتم تجاوزها. للمسار التشغيلي الكامل راجع `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### ملاحظة حول تفويض DNS العام

Le fichier zonefile est situé dans la zone de stockage. ما زلت بحاجة لضبط تفويض NS/DS للمنطقة الام لدى المسجل او مزود DNS حتى يتمكن الانترنت العام من العثور على خوادم الاسماء.- Le cutover est un apex/TLD comme ALIAS/ANAME (en anglais) et une version A/AAAA d'anycast en mode anycast.
- CNAME est un joli hôte (`<fqdn>.gw.sora.name`).
- المضيف القانوني (`<hash>.gw.sora.id`) يبقى تحت نطاق البوابة ولا يُنشر داخل نطاقك العام.

### قالب رؤوس البوابة

ينتج مساعد النشر ايضا `portal.gateway.headers.txt` و`portal.gateway.binding.json` et artefacts تلبي متطلبات DG-3 لربط محتوى البوابة:

- يحتوي `portal.gateway.headers.txt` pour le protocole HTTP (pour `Sora-Name` et `Sora-Content-CID` et `Sora-Proof` et CSP et HSTS و`Sora-Route-Binding`) والتي يجب على بوابات الحافة لصقها في كل رد.
- يسجل `portal.gateway.binding.json` المعلومات نفسها بشكل مقروء آليا حتى تتمكن تذاكر التغيير والاتمتة من مقارنة host/cid Il s'agit d'un shell.

Il s'agit d'un alias `cargo xtask soradns-binding-template` et d'un nom d'hôte correspondant à `sorafs-pin-release.sh`. لاعادة التوليد او التخصيص:

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

Vous pouvez utiliser CDN et JSON pour créer un lien vers une application Web. مع ادلة الاصدار.

يشغّل سكربت الاصدار مساعد التحقق تلقائيا حتى تحتوي تذاكر DG-3 على ادلة حديثة. Vous pouvez utiliser la version JSON :

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```Utilisez le code `Sora-Proof` pour utiliser le nom d'hôte `Sora-Proof` et le nom d'hôte du CID. وتفشل اذا انحرف اي رأس. احتفظ بمخرجات الكونسول مع باقي artefacts عند تشغيل الامر خارج CI.

> **تكامل واصف DNS:** يدمج `portal.dns-cutover.json` الان قسما `gateway_binding` يشير الى هذه الملفات (المسارات وcontent CID وحالة proof والقالب النصي للرؤوس) **وكذلك** مقطع `route_plan` الذي يشير الى `gateway.route_plan.json` وقوالب الرؤوس الرئيسية/التراجع. ادرج هذه الاقسام في كل DG-3 ليتمكن المراجعون من مقارنة قيم `Sora-Name/Sora-Proof/CSP` والتاكد من ان خطط الترقية/التراجع تطابق حزمة الادلة دون فتح الارشيف.

## الخطوة 9 - تشغيل مراقبات النشر

يتطلب بند خارطة الطريق **DOCS-3c** ادلة مستمرة على ان البوابة ووكيل Essayez-le وروابط البوابة تظل سليمة بعد النشر. شغّل المراقب الموحد مباشرة بعد الخطوتين 7-8 واربطه بمسباراتك المجدولة:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- يقوم `scripts/monitor-publishing.mjs` pour le produit (راجع `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) et les détails suivants : Utilisez CSP/Permissions-Policy, essayez-le (`/metrics`) et essayez-le (`cargo xtask soradns-verify-binding`) ici. Le Sora-Content-CID est un alias/manifeste.
- ينهي الامر بخروج غير صفري عند فشل اي sonde حتى تتمكن CI/cron/فرق التشغيل من ايقاف الترقية.
- Utilisez `--json-out` pour utiliser JSON et pour و`--evidence-dir` و`summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` حتى يتمكن مراجعو الحوكمة من مقارنة النتائج دون اعادة التشغيل. Il s'agit du bundle `artifacts/sorafs/<tag>/monitoring/` avec le bundle Sigstore et du descripteur de basculement DNS.
- ارفق مخرجات المراقبة وتصدير Grafana (`dashboards/grafana/docs_portal.json`) et Alertmanager Drill pour la connexion SLO avec DOCS-3c قابلا للتدقيق لاحقا. Il s'agit d'un fichier `docs/portal/docs/devportal/publishing-monitoring.md`.

La connexion HTTPS avec le code `http://` est également compatible avec le code `allowInsecureHttp`. حافظ على TLS في بيئات الانتاج/التجربة وفعّل الاستثناء فقط للمعاينات المحلية.

Vous pouvez utiliser `npm run monitor:publishing` pour Buildkite/cron. نفس الامر مع روابط الانتاج يغذي فحوصات الصحة المستمرة بين الاصدارات.

## الاتمتة عبر `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` pour les sections 2-6. C'est :1. Téléchargez `build/` dans l'archive tar.
2. Pour `car pack` et `manifest build` et `manifest sign` et `manifest verify-signature` et `proof verify`,
3. Utilisez `manifest submit` pour la liaison (pour la liaison d'alias) et utilisez Torii.
4. `artifacts/sorafs/portal.pin.report.json` et `portal.pin.proposal.json` et le descripteur de basculement DNS sont également utilisés (`portal.gateway.binding.json` pour le descripteur de basculement DNS) تتمكن فرق الحوكمة والشبكات والعمليات من مراجعة الادلة دون قراءة سجلات CI.

اضبط `PIN_ALIAS` و`PIN_ALIAS_NAMESPACE` و`PIN_ALIAS_NAME` و(اختياريا) `PIN_ALIAS_PROOF_PATH` قبل تشغيل السكربت. استخدم `--skip-submit` pour le transfert de données Le flux de travail sur GitHub est basé sur `perform_submit`.

## الخطوة 8 - نشر مواصفات OpenAPI et SBOM

Il s'agit de DOCS-7 et de OpenAPI, ainsi que du SBOM. تغطي الادوات الحالية الثلاثة:

1. **اعادة توليد وتوقيع المواصفات.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Utilisez `--version=<label>` pour obtenir le code `2025-q3`. `static/openapi/versions/<label>/torii.json` et `versions/current` et `static/openapi/versions.json` pour SHA-256 وحالة المانيفست وتاريخ التحديث. يقرأ موقع المطورين هذا المؤشر ليعرض اختيار النسخة ومعلومات البصمة/التوقيع. اذا تم حذف `--version` فسيبقي التسميات السابقة ويحدث `current` و`latest` فقط.

   Les paramètres SHA-256/BLAKE3 sont également compatibles avec `Sora-Proof` ou `/reference/torii-swagger`.2. **اصدار CycloneDX SBOMs.** يتوقع مسار الاصدار SBOMs عبر syft حسب `docs/source/sorafs_release_pipeline_plan.md`. احتفظ بالمخرجات قرب artefacts البناء :

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **تغليف كل payload داخل CAR.**

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

   Utilisez les alias `manifest build` / `manifest sign` pour les artefacts (avec `docs-openapi.sora` للمواصفات و`docs-sbom.sora` لحزمة SBOM الموقعة). Il s'agit de SoraDNS et de GAR qui utilisent la charge utile.

4. **الارسال والربط.** اعادة استخدام authority et Sigstore bundle, مع تسجيل alias tuple في قائمة فحص الاصدار حتى يعرف المدققون اي اسم Sora يقابل اي بصمة.

يساعد حفظ مانيifestات OpenAPI/SBOM مع بناء البوابة على ان يحتوي كل ticket على مجموعة الادلة الكاملة دون Je suis un emballeur.

### مساعد الاتمتة (script CI/package)

`./ci/package_docs_portal_sorafs.sh` يجمع الخطوات 1-8 في امر واحد. يقوم المساعد بـ:

- Fonctionnalités (`npm ci` et widgets OpenAPI/Norito).
- Les CARs sont associés à OpenAPI et SBOM à `sorafs_cli`.
- Utiliser `sorafs_cli proof verify` (`--proof`) et Sigstore (`--sign`, `--sigstore-provider`, `--sigstore-audience`) اختياريا.
- وضع كل الاصول تحت `artifacts/devportal/sorafs/<timestamp>/` وكتابة `package_summary.json` لادوات CI/release.
- تحديث `artifacts/devportal/sorafs/latest` ليشير الى اخر تشغيل.

مثال (خط كامل مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

اهم الاعلام:- `--out <dir>` - تغيير جذر الاصول (horodatage de la fonction).
- `--skip-build` - اعادة استخدام `docs/portal/build` الموجود.
- `--skip-sync-openapi` - `npm run sync-openapi` est compatible avec `cargo xtask openapi` pour crates.io.
- `--skip-sbom` - عدم استدعاء `syft` عند عدم توفره (مع تحذير).
- `--proof` - تنفيذ `sorafs_cli proof verify` لكل زوج CAR/manifeste.
- `--sign` - Voir `sorafs_cli manifest sign`.

Pour cela, utilisez `docs/portal/scripts/sorafs-pin-release.sh`. يجمع portal/OpenAPI/SBOM ويوقّع المانيفستات et métadonnées اضافية في `portal.additional_assets.json`. يدعم مفاتيح `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` alias لكل artefact وتجاوز مصدر SBOM عبر `--openapi-sbom-source` Il s'agit de charges utiles (`--skip-openapi`/`--skip-sbom`) et de `syft` pour `--syft-bin`.

يعرض السكربت كل الاوامر؛ La méthode `package_summary.json` est utilisée pour digérer les métadonnées et les métadonnées du shell.

## الخطوة 9 - Passerelle complète + SoraDNS

Pour le cutover, les alias et les preuves SoraDNS et les autres preuves sont :

1. **Porte de sonde.** pour `ci/check_sorafs_gateway_probe.sh` pour `cargo xtask sorafs-gateway-probe` pour les luminaires `fixtures/sorafs_gateway/probe_demo/`. للتشغيل الفعلي، وجّه الاختبار الى الـ hostname المطلوب :

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Capteur de sonde `Sora-Name` et `Sora-Proof` et `Sora-Proof-Status` et `docs/source/sorafs_alias_policy.md` pour la connexion à Internet Et les TTL et les liaisons GAR.Pour les contrôles ponctuels légers (par exemple, lorsque seul le lot de liaisons
   modifié), exécutez `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   L'assistant valide le bundle de liaison capturé et est pratique pour la libération
   des billets qui nécessitent uniquement une confirmation contraignante au lieu d’un exercice d’enquête complet.

2. **جمع الادلة drills.** الستخدم `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` لتجميع الرؤوس والسجلات في `artifacts/sorafs_gateway_probe/<stamp>/` وتحديث `ops/drill-log.md`, مع Il existe des hooks de restauration et PagerDuty. Utilisez `--host docs.sora` pour SoraDNS.

3. **التحقق من DNS liaisons.** عند نشر proof الخاص بالalias, احفظ ملف GAR المشار اليه وارفقة مع ادلة الاصدار. Le résolveur est utilisé pour `tools/soradns-resolver`. JSON utilise `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` pour le mappage d'hôte, les métadonnées et les étiquettes de télémétrie. يمكن للمساعد اصدار ملخص `--json-out` by GAR الموقّع.
  عند صياغة GAR جديد، فضّل `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`, ولا تستخدم `--manifest-cid` الا عند غياب ملف المانيفست. CID et BLAKE3 sont compatibles avec JSON et les labels et les étiquettes CSP/HSTS/Permissions-Policy est également disponible.

4. **مراقبة مقاييس alias.** ابق `torii_sorafs_alias_cache_refresh_duration_ms` et `torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة؛ Il s'agit d'un `dashboards/grafana/docs_portal.json`.

## الخطوة 10 - المراقبة وحزم الادلة- **الوحات.** صدّر `dashboards/grafana/docs_portal.json` و`dashboards/grafana/sorafs_gateway_observability.json` و`dashboards/grafana/sorafs_fetch_observability.json` لكل اصدار وارفقة مع التذكرة.
- **Sondes sondes.** Ajout de `artifacts/sorafs_gateway_probe/<stamp>/` à git-annex et à la version ultérieure.
- **حزمة الاصدار.** خزّن ملخصات CAR ومجموعات المانيفست وتواقيع Sigstore و`portal.pin.report.json` وسجلات Try-It وتقارير الروابط تحت مجلد واحد بتاريخ.
- **Perceuses.** Pour les perceuses, utilisez `scripts/telemetry/run_sorafs_gateway_probe.sh` et `ops/drill-log.md` pour SNNet-5.
- **روابط التذاكر.** اربط معرفات لوحات Grafana او صادرات PNG مع مسار تقرير sonde حتى يتمكن المراجعون من التحقق دون وصول coquille.

## الخطوة 11 - تمرين récupérer le tableau de bord de متعدد المصادر وادلة

Utilisez SoraFS pour récupérer la passerelle (DOCS-7/SF-6) vers DNS/passerelle. بعد تثبيت المانيفست:

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

   - احصل اولا على supplier adverts المشار اليها في المانيفست (مثل `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) et `--provider-advert name=path` لضمان تقييم القدرات شكل حتمي. استخدم `--allow-implicit-provider-metadata` **فقط** عند اعادة تشغيل luminaires pour CI.
   - Vous avez besoin de plus de cache/alias.

2. **ارشفة المخرجات.** خزّن `scoreboard.json` و`providers.ndjson` و`fetch.json` و`chunk_receipts.ndjson` في دليل الادلة.3. **Rechercher des informations.** Ajouter des informations sur **SoraFS Récupérer l'observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`) ci-dessous `torii_sorafs_fetch_duration_ms`/`_failures_total`.

4. **فحص قواعد التنبيه.** Utilisez `scripts/telemetry/test_sorafs_fetch_alerts.sh` pour promtool.

5. **الدمج في CI.** فعّل خطوة `sorafs_fetch` عبر `perform_fetch_probe` في staging/production لتوليد الادلة تلقائيا.

## الترقية والملاحظة والتراجع

1. **الترقية :** احتفظ بـ alias منفصلة لـ staging et production. Il s'agit d'un alias `manifest submit` qui correspond à `--alias-namespace/--alias-name`.
2. **المراقبة:** استخدم `docs/source/grafana_sorafs_pin_registry.json` ومسبارات البوابة الخاصة في `docs/portal/docs/devportal/observability.md`.
3. **التراجع :** لاعادة التراجع، اعد ارسال المانيفست السابق (او اسحب alias الحالي) عبر `sorafs_cli manifest submit --alias ... --retire`.

## قالب CI

يجب ان يتضمن الحد الادنى من خط الانابيب:

1. Build + Lint (`npm ci`, `npm run build`, sommes de contrôle).
2. التغليف (`car pack`) et المانيفست.
3. Jeton OIDC pour le jeton (`manifest sign`).
4. رفع الاصول (CAR, مانيفست, bundle, plan, résumés).
5. Registre des broches de votre choix :
   - Demandes d'extraction -> `docs-preview.sora`.
   - Tags / فروع محمية -> ترقية alias الانتاج.
6. تشغيل sondes + بوابات التحقق من preuve قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` يربط هذه الخطوات لاصدارات يدوية. Voici le workflow :- بناء/اختبار البوابة،
- تغليف build عبر `scripts/sorafs-pin-release.sh`,
- توقيع/تحقق manifest bundle sur GitHub OIDC,
- رفع CAR/manifest/bundle/plan/proof résumés كـ artefacts،
- (اختياريا) ارسال المانيفست وربط alias عند توفر secrets.

اضبط secrets/variables التالية قبل التشغيل :

| Nom | الغرض |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Utilisez Torii pour `/v2/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف époque المسجل مع الارسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | سلطة التوقيع لارسال المانيفست. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tuple est `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | bundle proof للalias en base64 (اختياري). |
| `DOCS_ANALYTICS_*` | نقاط Analytics/Probe الحالية. |

شغّل الـ workflow عبر واجهة Actions :

1. Pour `alias_label` (pour `docs.sora.link`) et pour `proposal_alias` et pour `release_tag`.
2. Utilisez `perform_submit` pour utiliser le Torii (exécution à sec) et utilisez l'alias.

يبقى `docs/source/sorafs_ci_templates.md` كمرجع لقوالب عامة، لكن workflow الخاص بالبوابة هو المسار المفضل للاصدارات اليومية.

## قائمة التحقق-[ ] نجاح `npm run build` et `npm run test:*` et `npm run check:links`.
-[ ] حفظ `build/checksums.sha256` et `build/release.json` ضمن الاصول.
- [ ] توليد CAR وplan وmanifeste وrésumé تحت `artifacts/`.
- [ ] Le bundle Sigstore est également disponible.
-[ ] حفظ `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json` عند الارسال.
- [ ] ارشفة `portal.pin.report.json` (و`portal.pin.proposal.json` عند توفرها).
- [ ] حفظ سجلات `proof verify` et `manifest verify-signature`.
- [ ] تحديث لوحات Grafana et sondes Try-It.
- [ ] ارفاق ملاحظات التراجع (ID المانيفست السابق + digest alias) مع تذكرة الاصدار.