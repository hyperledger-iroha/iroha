---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## نظرة عامة

Você pode usar o **DOCS-7** (SoraFS) e o **DOCS-8** (o pino do CI/CD) para fazer isso. عملي لبوابة المطورين. Para construir / lint, use SoraFS, instale o Sigstore, instale o arquivo Sigstore, A máquina de lavar roupa pode ser usada para lavar roupas e roupas.

O endpoint `sorafs_cli` (ou seja, `--features cli`) e o endpoint Torii são iguais Para alterar o registro de pinos, você pode substituir OIDC por Sigstore. Você pode usar o código de barras (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, número Torii) em CI, O que acontece é que as exportações são feitas através do shell.

## المتطلبات المسبقة

- Nó 18.18 e `npm` e `pnpm`.
- `sorafs_cli` em `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- O Torii é o `/v2/sorafs/*`, o que significa que você pode usar o cabo de segurança para obter mais informações. المستعارة.
- Use OIDC (GitHub Actions, GitLab, identidade de carga de trabalho), como `SIGSTORE_ID_TOKEN`.
- `examples/sorafs_cli_quickstart.sh` para fluxos de trabalho no GitHub/GitLab.
- اضبط متغيرات OAuth الخاصة بـ Try it (`DOCS_OAUTH_*`) وشغّل
  [lista de verificação de reforço de segurança](./security-hardening.md) para construir o build خارج المختبر. Você pode usar o recurso TTL/polling e usar o TTL/polling para obter mais informações. Use `DOCS_OAUTH_ALLOW_INSECURE=1` para remover o problema. Faça isso com uma chave de fenda.

## الخطوة 0 - التقاط حزمة وكيل Experimente

قبل ترقية المعاينة الى Netlify او البوابة, ثبّت مصادر وكيل Try it وبصمة المانيفست OpenAPI الموقّع في Veja mais:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

O `scripts/tryit-proxy-release.mjs` é um proxy/sonda/reversão, e o OpenAPI e o `release.json` são `checksums.sha256`. ارفق هذه الحزمة مع تذكرة ترقية Netlify/SoraFS gateway كي يتمكن المراجعون من اعادة تشغيل مصادر O código de barras Torii está danificado. تسجّل الحزمة ايضا ما اذا كان تم تمكين bearer tokens المرسلة من العميل (`allow_client_auth`) للحفاظ على اتساق خطة Descrição e CSP.

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
```

Para `npm run build`, use `scripts/write-checksums.mjs` e:

- `build/checksums.sha256` - O SHA256 é compatível com `sha256sum -c`.
- `build/release.json` - بيانات وصفية (`tag`, `generated_at`, `source`) no carro/carro.

احفظ كلا الملفين مع ملخص CAR حتى يتمكن المراجعون من مقارنة نواتج المعاينة بدون اعادة بناء.

## الخطوة 2 - تغليف الاصول الثابتة

شغّل CAR packer مقابل مخرجات Docusaurus. O código de segurança é `artifacts/devportal/`.

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

O JSON contém pedaços, resumos e resumos, além de prova de `manifest build` e CI استخدامها Não.

## Passo 2b - تغليف مرافقات OpenAPI e SBOMO DOCS-7 está disponível para download e OpenAPI e SBOM e SBOM. Use o artefato `Sora-Proof`/`Sora-Content-CID`. Você pode usar o OpenAPI (`static/openapi/`) e SBOM para obter mais informações. Você pode usar `syft` em CARs `openapi.*`/`*-sbom.*` e usar carros em `artifacts/sorafs/portal.additional_assets.json`. عند اتباع المسار اليدوي, اعد الخطوات 2-4 لكل payload مع بادئاتها ووسوم metadata الخاصة بها (مثلا `--car-out "$OUT"/openapi.car` exemplo `--metadata alias_label=docs.sora.link/openapi`). Use o manifest/alias em Torii (OpenAPI, SBOM البوابة, SBOM OpenAPI) para DNS حتى تتمكن البوابة من تقديم provas مدبسة لكل artefato منشور.

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
```

عدّل اعلام pin-policy بما يتناسب مع نافذة الاصدار (مثلا `--pin-storage-class hot` للكاناري). O JSON é definido como o valor do arquivo.

## Passo 4 - Passo Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

O pacote inclui o token OIDC e o token BLAKE3 e o JWT. احتفظ بالـ bundle والتوقيع المنفصل؛ Não se preocupe, você pode fazer isso sem problemas. Você pode usar o `--identity-token-env` (e o `SIGSTORE_ID_TOKEN` no site) para obter mais informações. Verifique se o OIDC está danificado.

## الخطوة 5 - Registro de pin do الارسال الى

Coloque os pedaços de papel (e os pedaços) em Torii. O resumo do resumo é um resumo do texto/resumo do documento.

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

عند اطلاق معاينة او كاناري (`docs-preview.sora`), اعد الارسال باستخدام alias مختلف حتى يتمكن QA من Você pode fazer isso sem problemas.

Verifique o valor do arquivo: `--alias-namespace` e `--alias-name` e `--alias-proof`. Esta é a prova de pacote (base64 e Norito bytes) عند الموافقة؛ Verifique se o CI está conectado ao `manifest submit`. اترك اعلام alias فارغة اذا كنت تريد تثبيت المانيفست بدون تغيير DNS.

## الخطوة 5b - توليد مقترح حوكمة

يجب ان يرافق كل مانيفست مقترح جاهز للبرلمان بحيث يستطيع اي مواطن من Sora تقديم التغيير دون امتلاك بيانات اعتماد مميزة. Você pode enviar/assinar aqui:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

يلتقط `portal.pin.proposal.json` التعليمة القياسية `RegisterPinManifest` وبصمة الـ chunks والسياسة وتلميح alias. ارفقه مع تذكرة الحوكمة او بوابة البرلمان كي يتمكن المندوبون من مراجعة الـ payload دون اعادة بناء. Se você não usar o Torii, não poderá usar nenhum produto.

## الخطوة 6 - التحقق من provas e والقياسات

بعد التثبيت, نفذ خطوات التحقق الحتمية:

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

- راقب `torii_sorafs_gateway_refusals_total` e `torii_sorafs_replication_sla_total{outcome="missed"}` são necessários.
- Verifique `npm run probe:portal` e experimente o Try-It.
- اجمع ادلة المراقبة المذكورة في
  [Publicação e monitoramento](./publishing-monitoring.md) لتلبية بوابة الملاحظة DOCS-3c. Você pode usar o `bindings` (OpenAPI, SBOM para `bindings`, SBOM para OpenAPI) e `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` pode ser usado para substituir `hostname`. O nome do arquivo JSON é definido como JSON (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) Para obter mais informações:```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الخطوة 6a - تخطيط شهادات البوابة

O desafio TLS SAN/challenge é definido como GAR para acessar o site e o servidor DNS. يعكس المساعد الجديد مدخلات DG-3 عبر تعداد المضيفين قانونيين wildcard e SAN para pretty-host e DNS-01 والتحديات Mais informações:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Isso significa que o JSON é o mais importante (e o valor do JSON é definido) para que você possa usá-lo em SAN. اعدادات `torii.sorafs_gateway.acme` ويتأكد مراجعو GAR من الخرائط canonical/pretty دون اعادة الحساب. Use `--name` para que o dispositivo não funcione corretamente.

## الخطوة 6b - اشتقاق خرائط المضيفين القانونية

قبل توليد قوالب GAR, سجّل خريطة المضيف الحتمية لكل alias. O `cargo xtask soradns-hosts` é definido como `--name` e o cartão curinga (`<base32>.gw.sora.id`) é o curinga. (`*.gw.sora.id`) é um código de erro (`<alias>.gw.sora.name`). احفظ الناتج ضمن artefatos الاصدار كي يتمكن مراجعو DG-3 من مقارنة الخريطة مع ارسال GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Use `--verify-host-patterns <file>` para definir o valor do arquivo GAR ou JSON. يقبل المساعد عدة ملفات للتحقق, مما يسهل تدقيق قالب GAR e `portal.gateway.binding.json` na próxima página:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

ارفق ملخص JSON وسجل التحقق مع تذكرة تغيير DNS/البوابة حتى يستطيع المدققون تأكيد المضيفين Não há nenhum problema com isso. اعد تشغيل الامر عند اضافة alias جديد حتى ترث تحديثات GAR نفس الادلة.

## Passo 7 - Corte de DNS

Verifique se o produto está funcionando corretamente. بعد نجاح الارسال (ربط الاسم المستعار), يصدر المساعد
`artifacts/sorafs/portal.dns-cutover.json`, descrição:

- بيانات ربط الاسم المستعار (namespace/nome/prova, بصمة المانيفست, عنوان Torii, epoch الارسال, السلطة).
- سياق الاصدار (tag, rótulo de alias, مسارات المانيفست/CAR, خطة الـ pedaços, pacote Sigstore).
- مؤشرات التحقق (Teste de teste, alias + endpoint Torii).
- حقول تغيير اختيارية (معرف التذكرة, نافذة cutover, جهة الاتصال التشغيلية, مضيف/منطقة الانتاج).
- بيانات ترقية المسار المشتقة من رأس `Sora-Route-Binding` (المضيف القانوني/CID, مسارات الرأس + الربط, اوامر التحقق) لضمان ان ترقية GAR e تمارين التراجع تشير لنفس الادلة.
- ملفات route-plan المولدة (`gateway.route_plan.json`, قوالب الرؤوس, ورؤوس التراجع الاختيارية) حتى تتحقق تذاكر DG-3 e lint é um CI que pode ser usado/definido.
- بيانات اختيارية لالغاء الكاش (purge, متغير auth, حمولة JSON, ومثال `curl`).
- تلميحات التراجع التي تشير الى الوصف السابق (وسم الاصدار وبصمة المانيفست) لابقاء المسار الحتمي Não.

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

ارفق `portal.cache_plan.json` بحزمة DG-3 ليحصل المشغلون على مسارات/مضيفين حتميين (e مرشحات auth) عند ارسال Modelo `PURGE`. Não há nada que você possa fazer com isso.

تحتاج حزمة DG-3 ايضا الى قائمة فحص للترقية والتراجع. Solução de problemas `cargo xtask soradns-route-plan`:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

يسجل `gateway.route_plan.json` المضيفين القانونيين/الجميلين, تذكيرات فحوص الصحة المرحلية, تحديثات البط GAR, purge للكاش, وخطوات التراجع. ارفقه مع ملفات GAR/binding/cutover قبل ارسال التذكرة حتى يتمكن فريق Ops من التدريب على الخطوات نفسها.O `scripts/generate-dns-cutover-plan.mjs` é substituído por `sorafs-pin-release.sh`. Leia mais:

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
| `DNS_CUTOVER_WINDOW` | Corte de corte padrão ISO8601 (modelo `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | مضيف الانتاج + المنطقة المعتمدة. |
| `DNS_OPS_CONTACT` | Deixe-o cair. |
| `DNS_CACHE_PURGE_ENDPOINT` | Não purgue o dispositivo no final. |
| `DNS_CACHE_PURGE_AUTH_ENV` | A remoção do token é realizada (como `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | مسار الوصف السابق لبيانات التراجع. |

JSON é uma ferramenta de DNS que pode ser usada para configurar o DNS A sonda de teste não é compatível com CI. Use CLI para `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact` e `--cache-purge-endpoint` و`--cache-purge-auth-env` و`--previous-dns-plan` نفس امكانية التخصيص خارج CI.

## الخطوة 8 - اصدار هيكل zonefile للمحلل (اختياري)

عند تحديد نافذة القطع للانتاج, يمكن لسكريبت الاصدار ان يصدر هيكل zonefile, SNS e snippet para Então. Crie um DNS e um serviço de gerenciamento de arquivos DNS e clique em CLI; O código `scripts/sns_zonefile_skeleton.py` pode ser danificado. وفر على الاقل قيمة A/AAAA/CNAME وبصمة GAR (BLAKE3-256 للحمولة الموقعة). Para obter mais informações sobre o `--dns-zonefile-out`, use o `artifacts/sns/zonefiles/<zone>/<hostname>.json` ou `artifacts/sns/zonefiles/<zone>/<hostname>.json`. `ops/soradns/static_zones.<hostname>.json` é o trecho do resolvedor.

| المتغير / الخيار | الغرض |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | É o esqueleto do zonefile. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Resolva o snippet do resolvedor (`ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL é uma taxa de câmbio (valor: 600 dólares). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Não é IPv4 (é possível usar o IPv4 e usá-lo). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Sem IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | É CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Use SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Arquivo TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Etiqueta تجاوز النسخة المحسوب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | O carimbo de data/hora `effective_at` (RFC3339) não é válido. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Verifique a prova do site. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Verifique o CID do site. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | حالة تجميد الحارس (suave, duro, descongelamento, monitoramento, emergência). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | مرجع تذكرة التجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | timestamp RFC3339 para descongelamento. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | ملاحظات تجميد اضافية (قائمة مفصولة بفواصل). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Use BLAKE3-256 (hex) para definir o valor. Não há ligações e ligações para isso. |

Use o GitHub Actions para definir o pino dos artefatos. اضبط الاسرار التالية (يمكن ان تحتوي القيم على قوائم مفصولة بفواصل):| Segredo | الغرض |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | مضيف/منطقة الانتاج للمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | جهة الاتصال المخزنة في الوصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Não use IPv4/IPv6. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | É CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Use SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | سجلات TXT اضافية. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Você pode fazer isso. |
| `DOCS_SORAFS_GAR_DIGEST` | Ative BLAKE3 para ativar a função. |

Use `.github/workflows/docs-portal-sorafs-pin.yml` e `dns_change_ticket` e `dns_cutover_window` para obter o arquivo/zonefile. الصحيحة. Certifique-se de que o funcionamento a seco seja executado.

مثال نموذجي (runbook do proprietário do SN-7):

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

Faça o download do arquivo TXT ou TXT para que você possa usar o `effective_at` Bem. Verifique o número de telefone `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Como remover o nome do DNS

O zonefile não está disponível para download. Você pode usar o NS/DS para configurar o servidor e o DNS para obter mais informações على خوادم الاسماء.

- Transmitir cutover para apex/TLD, usar ALIAS/ANAME (حسب المزود) e transferir A/AAAA para Anycast Anycast. Então.
- للنطاقات الفرعية, انشر CNAME الى الـ beautiful host المشتق (`<fqdn>.gw.sora.name`).
- O código de barras (`<hash>.gw.sora.id`).

### قالب رؤوس البوابة

ينتج مساعد النشر ايضا `portal.gateway.headers.txt` و`portal.gateway.binding.json` وهما artefatos تلبي متطلبات DG-3 لربط محتوى البوابة:

- Use `portal.gateway.headers.txt` para usar o protocolo HTTP (ou seja, `Sora-Name`, `Sora-Content-CID`, `Sora-Proof` e CSP وHSTS و`Sora-Route-Binding`).
- يسجل `portal.gateway.binding.json` معلومات نفسها بشكل مقروء آليا حتى تتمكن تذاكر التغيير والاتمتة من O host/cid é o mesmo que o shell.

يتم توليدها تلقائيا عبر `cargo xtask soradns-binding-template` e alias e nome de host `sorafs-pin-release.sh`. Leia mais e leia:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

O `--csp-template` e o `--permissions-template` e o `--hsts-template` são usados para obter informações sobre o produto. Use o `--no-*` para remover o problema.

Você pode usar CDN e JSON para obter informações sobre como criar uma conta ترقية المضيف مع ادلة الاصدار.

يشغّل سكربت الاصدار مساعد التحقق تلقائيا حتى تحتوي تذاكر DG-3 على ادلة حديثة. Para obter mais informações sobre o formato JSON:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Você pode usar o `Sora-Proof` para obter informações sobre o CID `Sora-Route-Binding`. O nome do host é o nome do host e o nome do host. احتفظ بمخرجات الكونسول مع باقي artefatos عند تشغيل الامر خارج CI.

> **تكامل واصف DNS:** يدمج `portal.dns-cutover.json` الان قسما `gateway_binding` يشير الى هذه الملفات (المسارات وcontent CID وحالة prova والقالب النصي للرؤوس) **وكذلك** مقطع `route_plan` الذي يشير الى `gateway.route_plan.json` وقوالب الرؤوس الرئيسية/التراجع. ادرج هذه الاقسام في كل تذكرة DG-3 ليتمكن مراجعون من مقارنة قيم `Sora-Name/Sora-Proof/CSP` والتاكد من ان خطط الترقية/التراجع تطابق حزمة الادلة دون فتح الارشيف.

## الخطوة 9 - تشغيل مراقبات النشريتطلب بند خارطة الطريق **DOCS-3c** ادلة مستمرة على ان البوابة ووكيل Experimente وروابط البوابة تظل سليمة بعد النشر. شغّل المراقب الموحد مباشرة بعد الخطوتين 7-8 e بمسباراتك المجدولة:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- يقوم `scripts/monitor-publishing.mjs` بتحميل ملف الاعداد (راجع `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) وتنفيذ ثلاث فحوصات: مسارات Você pode usar CSP/Permissions-Policy, tentar e experimentá-lo (`/metrics`), e fazer o teste (`cargo xtask soradns-verify-binding`) O nome do arquivo Sora-Content-CID é o alias/manifest.
- Não há problema em que o probe seja configurado para CI/cron/فرق التشغيل من ايقاف الترقية.
- Atualizar `--json-out` para JSON e JSON و`--evidence-dir` يصدر `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` حتى يتمكن Faça uma pausa no carro e não use nada. Você pode usar `artifacts/sorafs/<tag>/monitoring/` com pacote Sigstore e descritor de substituição de DNS.
- ارفق مخرجات المراقبة وتصدير Grafana (`dashboards/grafana/docs_portal.json`) e معرف Alertmanager drill بتذكرة الاصدار ليكون SLO الخاص O DOCS-3c é compatível com o sistema. Você pode usar o produto em `docs/portal/docs/devportal/publishing-monitoring.md`.

Verifique se o HTTPS está no `http://` e se você estiver usando o `allowInsecureHttp` neste site. Você pode usar TLS no site/serviço e no site da empresa.

Use `npm run monitor:publishing` para construir o Buildkite/cron. Não há nenhum problema em que você possa usá-lo.

## Nome de usuário `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` يجمع الخطوات 2-6. É:

1. Use `build/` no tarball,
2. `car pack` e `manifest build` e `manifest sign` e `manifest verify-signature` e `proof verify`,
3. ينفذ `manifest submit` اختياريا (بما فيه alias binding) عندما تتوفر بيانات اعتماد Torii,
4. Use `artifacts/sorafs/portal.pin.report.json` e `portal.pin.proposal.json` e descritor de corte de DNS e descreva o descritor de DNS (`portal.gateway.binding.json` de acordo com o padrão) حتى تتمكن فرق الحوكمة والشبكات والعمليات من مراجعة الادلة دون قراءة سجلات CI.

Use `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` e (اختياريا) `PIN_ALIAS_PROOF_PATH` para remover o problema. Use `--skip-submit` para o painel de controle O fluxo de trabalho no GitHub é baseado em `perform_submit`.

## الخطوة 8 - نشر مواصفات OpenAPI e SBOM

O DOCS-7 está disponível para download e OpenAPI e SBOM. تغطي الادوات الحالية الثلاثة:

1. **اعادة توليد وتوقيع المواصفات.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Use `--version=<label>` para remover o problema (como `2025-q3`). Você pode usar o `static/openapi/versions/<label>/torii.json` e o `versions/current` e o `static/openapi/versions.json`. SHA-256 é um arquivo de código aberto e gratuito. Não há nenhum problema em que você possa usar o telefone e o computador. Você pode usar `--version` para obter informações sobre o produto e `current` e `latest`.

   O SHA-256/BLAKE3 deve ser substituído por `Sora-Proof` para `/reference/torii-swagger`.

2. **Efetue SBOMs CycloneDX.** A instalação de SBOMs do CycloneDX é realizada por syft `docs/source/sorafs_release_pipeline_plan.md`. احتفظ بالمخرجات قرب artefatos البناء:

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
   ```Use o `manifest build` / `manifest sign` para usar o artefato (como `docs-openapi.sora` Para obter mais informações, consulte `docs-sbom.sora` para SBOM. O SoraDNS e o GAR podem ser usados ​​para carregar a carga útil.

4. **الارسال والربط.** اعادة استخدام autoridade e pacote Sigstore, مع تسجيل alias tuple في قائمة فحص الاصدار Eu não sei o que fazer com Sora e Sora.

يساعد حفظ مانيifestات OpenAPI/SBOM مع بناء البوابة على ان يحتوي كل ticket على مجموعة الادلة الكاملة Não há empacotador.

### مساعد الاتمتة (CI/script de pacote)

`./ci/package_docs_portal_sorafs.sh` é exibido de 1 a 8 de maio. O que é isso:

- اعداد البوابة (`npm ci`, مزامنة OpenAPI/Norito, widgets de configuração).
- اصدار CARs ومانيفستات للبوابة و OpenAPI و SBOM عبر `sorafs_cli`.
- `sorafs_cli proof verify` (`--proof`) e Sigstore (`--sign`, `--sigstore-provider`, `--sigstore-audience`) اختياريا.
- Você pode usar `artifacts/devportal/sorafs/<timestamp>/` e `package_summary.json` para CI/release.
- Verifique `artifacts/devportal/sorafs/latest` para remover o problema.

Exemplo (ou seja, Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Como fazer:

- `--out <dir>` - تغيير جذر الاصول (registro de data e hora).
- `--skip-build` - اعادة استخدام `docs/portal/build`.
- `--skip-sync-openapi` - Use `npm run sync-openapi` para encontrar `cargo xtask openapi` em crates.io.
- `--skip-sbom` - Você pode usar `syft` para obter mais informações (sem problemas).
- `--proof` - تنفيذ `sorafs_cli proof verify` para CAR/manifest.
- `--sign` - `sorafs_cli manifest sign`.

Use o `docs/portal/scripts/sorafs-pin-release.sh`. Acesse portal/OpenAPI/SBOM e metadados e metadados em `portal.additional_assets.json`. يدعم مفاتيح `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` لتعيين alias لكل artefato وتجاوز مصدر SBOM عبر `--openapi-sbom-source` suporta cargas úteis (`--skip-openapi`/`--skip-sbom`) e `syft` sem problemas `--syft-bin`.

يعرض السكربت كل الاوامر؛ Use o `package_summary.json` para obter informações sobre o shell de digests e metadados sim.

## Passo 9 - Passo a passo do gateway + SoraDNS

Para o cutover, o alias do SoraDNS e o SoraDNS são as provas:

1. **Porta da sonda.** O `ci/check_sorafs_gateway_probe.sh` é usado para `cargo xtask sorafs-gateway-probe` nos acessórios do `fixtures/sorafs_gateway/probe_demo/`. للتشغيل الفعلي, وجّه الاختبار الى الـ nome de host:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   A sonda é `Sora-Name` e `Sora-Proof` e `Sora-Proof-Status` e `docs/source/sorafs_alias_policy.md` e não está disponível Ligações e TTLs e GAR.

   Para verificações pontuais leves (por exemplo, quando apenas o pacote de ligação
   alterado), execute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   O auxiliar valida o pacote de ligação capturado e é útil para liberação
   tickets que precisam apenas de confirmação de vinculação em vez de um exercício de sondagem completo.

2. **Não faça exercícios.** Use `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` para usar o `artifacts/sorafs_gateway_probe/<stamp>/` e `ops/drill-log.md`, Você pode usar ganchos de reversão e PagerDuty. Use `--host docs.sora` para usar o SoraDNS.3. **التحقق من ligações DNS.** عند نشر prova الخاص بالalias, احفظ ملف GAR المشار اليه وارفقة مع ادلة الاصدار. يمكن لمشغلي resolver اعادة الاختبار عبر `tools/soradns-resolver`. O JSON é definido como `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` para mapeamento de host, metadados e rótulos de telemetria. Verifique se o `--json-out` é compatível com GAR.
  GAR جديد, فضّل `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`, ولا تستخدم `--manifest-cid` الا عند غياب ملف المانيفست. O CID e o BLAKE3 são transferidos para o JSON e para os rótulos e rótulos. A política CSP/HSTS/Permissions-Policy é a solução para o problema.

4. **مراقبة مقاييس alias.** ابق `torii_sorafs_alias_cache_refresh_duration_ms` e `torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة؛ A solução está em `dashboards/grafana/docs_portal.json`.

## الخطوة 10 - المراقبة وحزم الادلة

- **اللوحات.** Você pode usar `dashboards/grafana/docs_portal.json`, `dashboards/grafana/sorafs_gateway_observability.json` e `dashboards/grafana/sorafs_fetch_observability.json` para obter mais informações e mais informações.
- **Testes de teste.** O arquivo `artifacts/sorafs_gateway_probe/<stamp>/` está no git-annex e no arquivo .
- **حزمة الاصدار.** خزّن ملخصات CAR ومجموعات المانيفست وتواقيع Sigstore و`portal.pin.report.json` وسجلات Try-It وتقارير الروابط تحت مجلد واحد بتاريخ.
- **Brocas.** Você pode usar brocas, usando `scripts/telemetry/run_sorafs_gateway_probe.sh` ou `ops/drill-log.md` para usar o SNNet-5.
- **روابط التذاكر.** اربط معرفات لوحات Grafana او صادرات PNG مع مسار تقرير probe حتى يتمكن المراجعون Meu nome é shell.

## الخطوة 11 - تمرين buscar متعدد المصادر وادلة placar

Use SoraFS para fetch através do DNS/gateway. بعد تثبيت المانيفست:

1. **تشغيل `sorafs_fetch` على المانيفست الحي.** استخدم نفس plano/manifesto مع بيانات اعتماد البوابة لكل مزود واحفظ جميع المخرجات:

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

   - احصل اولا على Provider Ads المشار اليها في المانيفست (مثل `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) ومررها عبر `--provider-advert name=path` لضمان تقييم القدرات بشكل حتمي. Use `--allow-implicit-provider-metadata` **فقط** عند اعادة تشغيل fixtures em CI.
   - عند وجود مناطق اضافية, اعد التشغيل مع مزوديها للحصول على ادلة لكل cache/alias.

2. **Efetue o procedimento.** Insira `scoreboard.json` e `providers.ndjson` e `fetch.json` e `chunk_receipts.ndjson` no computador.

3. **تحديث القياسات.** استورد المخرجات في لوحة **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) وراقب `torii_sorafs_fetch_duration_ms`/`_failures_total`.

4. **Isso é feito.** Use `scripts/telemetry/test_sorafs_fetch_alerts.sh` e use o promtool.

5. **Depois de CI.** O valor do `sorafs_fetch` é o `perform_fetch_probe` para preparação/produção.

## الترقية والملاحظة والتراجع

1. **الترقية:** احتفظ بـ aliases منفصلة لـ preparação e produção. قم بالترقية عبر اعادة `manifest submit` بالمانيفست نفسه مع تبديل `--alias-namespace/--alias-name` الى alias الانتاج.
2. **Escolha:** Use `docs/source/grafana_sorafs_pin_registry.json` e insira o código de segurança em `docs/portal/docs/devportal/observability.md`.
3. **التراجع:** لاعادة التراجع, اعد ارسال المانيفست السابق (او اسحب alias الحالي) عبر `sorafs_cli manifest submit --alias ... --retire`.

## قالب CI

يجب ان يتضمن الحد الادنى من خط الانابيب:1. Build + lint (`npm ci`, `npm run build`, somas de verificação).
2. Verifique (`car pack`) e verifique o valor.
3. Selecione o token OIDC para o token `manifest sign`.
4. رفع الاصول (CAR, مانيفست, pacote, plano, resumos).
5. Registro de pinos do número:
   - Solicitações pull -> `docs-preview.sora`.
   - Tags / فروع محمية -> ترقية alias الانتاج.
6. Verifique as sondas + as provas da prova.

`.github/workflows/docs-portal-sorafs-pin.yml` não funciona corretamente. O fluxo de trabalho é:

- بناء/اختبار البوابة,
- Construir build em `scripts/sorafs-pin-release.sh`,
- Pacote de manifesto توقيع/تحقق no GitHub OIDC,
- رفع CAR/manifesto/pacote/plano/resumos de provas كـ artefatos،
- (اختياريا) ارسال المانيفست وربط alias عند توفر segredos.

Coloque segredos/variáveis no lugar:

| Nome | الغرض |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | O Torii é o mesmo do `/v2/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف época المسجل مع الارسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | سلطة التوقيع لارسال المانيفست. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tupla é igual a `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | bundleproof como alias بترميز base64 (اختياري). |
| `DOCS_ANALYTICS_*` | Não use analytics/probe. |

شغّل الـ fluxo de trabalho عبر واجهة Ações:

1. O `alias_label` (como o `docs.sora.link`), o `proposal_alias` e o `release_tag`.
2. Use `perform_submit` para executar o teste Torii (ensaio) e use o alias.

O `docs/source/sorafs_ci_templates.md` é usado para configurar o fluxo de trabalho do fluxo de trabalho para o local de trabalho.

## قائمة التحقق

- [ ] Nome `npm run build` e `npm run test:*` e `npm run check:links`.
- [ ] é `build/checksums.sha256` e `build/release.json`.
- [ ] توليد CAR وplano وmanifesto وResumo تحت `artifacts/`.
- [ ] حفظ Sigstore bundle والتوقيع المنفصل مع السجلات.
- [ ] é `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json`.
- [ ] `portal.pin.report.json` (e `portal.pin.proposal.json` عند توفرها).
- [ ] حفظ سجلات `proof verify` e `manifest verify-signature`.
- [ ] تحديث لوحات Grafana e sondas Try-It.
- [ ] ارفاق ملاحظات التراجع (ID المانيفست السابق + alias de resumo) مع تذكرة الاصدار.