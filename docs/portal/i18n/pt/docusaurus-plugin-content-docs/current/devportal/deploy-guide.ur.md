---
lang: pt
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
یہ build/lint مرحلہ, SoraFS پیکجنگ, Sigstore کے ذریعے مینی فیسٹ سائننگ,
alias پروموشن، توثیق، اور rollback ڈرلز کو کور کرتی ہے تاکہ ہر visualização اور
liberar قابلِ اعادہ اور قابلِ آڈٹ ہو۔

یہ فلو فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری (`--features cli` کے ساتھ
build (شدہ) ہے، pin-registry اجازتوں والے Torii endpoint تک رسائی ہے، اور
Sigstore کے لئے OIDC اسناد ہیں۔ طویل مدتی راز (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii ٹوکنز) کو اپنے CI والٹ میں رکھیں؛ لوکل رنز انہیں
exportações de shell سے لوڈ کر سکتے ہیں۔

## پیشگی شرائط

- Nó 18.18+ کے ساتھ `npm` یا `pnpm`.
- `sorafs_cli` ou `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے حاصل ہو۔
- Torii URL ou `/v1/sorafs/*` ظاہر کرے اور ایک اتھارٹی اکاؤنٹ/پرائیویٹ کی جو
  مینی فیسٹس اور aliases جمع کر سکے۔
- Emissor OIDC (GitHub Actions, GitLab, identidade de carga de trabalho وغیرہ) تاکہ
  `SIGSTORE_ID_TOKEN` منٹ کیا جا سکے۔
- Execução: testes de simulação em `examples/sorafs_cli_quickstart.sh` no GitHub/GitLab
  fluxos de trabalho são `docs/source/sorafs_ci_templates.md`.
- Experimente OAuth ویری ایبلز (`DOCS_OAUTH_*`) کنفیگر کریں اور build کو لیب سے باہر
  پروموٹ کرنے سے پہلے [lista de verificação de reforço de segurança](./security-hardening.md)
  چلائیں۔ اب اگر یہ ویری ایبلز موجود نہ ہوں یا TTL/botões de votação Não há necessidade de usar
  حدود سے باہر ہوں تو پورٹل build ناکام ہو جاتا ہے؛ `DOCS_OAUTH_ALLOW_INSECURE=1`
  صرف عارضی لوکل previews کے لئے exportar کریں۔ pen-test
  منسلک کریں۔

## مرحلہ 0 — Experimente

Netlify یا gateway پر visualização پروموٹ کرنے سے پہلے Experimente پروکسی سورسز اور
Aqui está OpenAPI مینی فیسٹ digest کو ایک متعین بنڈل میں اسٹیمپ کریں:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` پروکسی/probe/rollback helpers کاپی کرتا ہے،
Assinatura OpenAPI
`checksums.sha256` کھتا ہے۔ Como usar o gateway Netlify/SoraFS
ساتھ منسلک کریں تاکہ ریویورز عین وہی پروکسی سورسز اور Torii ٹارگٹ ہنٹس بغیر
دوبارہ build کے ری پلے کر سکیں۔ بنڈل یہ بھی ریکارڈ کرتا ہے کہ آیا fornecido pelo cliente
bearers são implementados (`allow_client_auth`) rollout de lançamento para CSP قواعد ہم آہنگ
رہیں۔

## Passo 1 — Construa build e lint

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

`npm run build` é um cartão de crédito `scripts/write-checksums.mjs` de alta qualidade
یہ تیار کرتا ہے:

- `build/checksums.sha256` — `sha256sum -c` کے لئے موزوں SHA256 مینی فیسٹ۔
- `build/release.json` — میٹا ڈیٹا (`tag`, `generated_at`, `source`) ou ہر
  CAR/manifesto میں پِن کیا جاتا ہے۔

دونوں فائلیں CAR سمری کے ساتھ آرکائیو کریں تاکہ ریویورز بغیر دوبارہ construir کیے
visualização

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

Contagens de blocos JSON, resumos e dicas de planejamento de prova محفوظ کرتا ہے جنہیں
`manifest build` é CI ڈیش بورڈز بعد میں دوبارہ استعمال کرتے ہیں۔

## مرحلہ 2b — OpenAPI اور SBOM معاون پیکج کریں

DOCS-7 تقاضہ کرتا ہے کہ پورٹل سائٹ, OpenAPI اسنیپ شاٹ, اور SBOM payloads کو
الگ الگ مینی فیسٹس کے طور پر شائع کیا جائے تاکہ gateways ہر آرٹیفیکٹ کے لئے
`Sora-Proof`/`Sora-Content-CID` ہیڈرز اسٹپل کر سکیں۔ ریلیز ہیلپر
(`scripts/sorafs-pin-release.sh`) پہلے ہی OpenAPI ڈائریکٹری (`static/openapi/`)
Para `syft`, você pode usar SBOMs e `openapi.*`/`*-sbom.*` CARs میں پیک
کرتا ہے اور میٹا ڈیٹا `artifacts/sorafs/portal.additional_assets.json` Cartão
ریکارڈ کرتا ہے۔ دستی فلو چلاتے وقت, ہر payload کے لئے مراحل 2-4 دہرائیں اور
Existem prefixos e rótulos de metadados.
`--car-out "$OUT"/openapi.car` é igual a `--metadata alias_label=docs.sora.link/openapi`).
DNS سوئچ کرنے سے پہلے ہر manifest/alias جوڑے کو Torii میں رجسٹر کریں (سائٹ،
OpenAPI, پورٹل SBOM, OpenAPI SBOM) تاکہ gateway تمام شائع شدہ آرٹیفیکٹس کے لئے
provas grampeadas فراہم کر سکے۔

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
canários کے لئے `--pin-storage-class hot`)۔ JSON ورژن اختیاری ہے مگر revisão de código
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
```bundle مینی فیسٹ digest, chunk digests, اور OIDC ٹوکن کا BLAKE3 hash ریکارڈ
کرتا ہے بغیر JWT محفوظ کیے۔ pacote ou assinatura desanexada
رکھیں؛ promoções de produção
کر سکتی ہیں۔ لوکل رنز میں provedor فلیگز کو `--identity-token-env` سے بدل سکتے
ہیں (یا ماحول میں `SIGSTORE_ID_TOKEN` سیٹ کر سکتے ہیں) جب کوئی بیرونی OIDC
ہیلپر ٹوکن جاری کرے۔

## مرحلہ 5 — pin رجسٹری میں جمع کرائیں

سائن شدہ مینی فیسٹ (plano de bloco) Torii کو جمع کرائیں۔ ہمیشہ resumo طلب کریں
تاکہ رجسٹری prova de entrada/alias قابلِ آڈٹ رہے۔

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

جب preview یا canary alias (`docs-preview.sora`) رول آؤٹ کریں تو منفرد alias کے
ساتھ submissão دوبارہ کریں تاکہ Produção de controle de qualidade پروموشن سے پہلے مواد کی تصدیق کر سکے۔

Ligação de alias کے لئے تین فیلڈز درکار ہیں: `--alias-namespace`, `--alias-name`,
Sobre `--alias-proof`۔ گورننس alias درخواست منظور ہونے پر pacote de prova (base64 یا
Norito bytes) تیار کرتی ہے؛ Segredos CI میں رکھیں اور `manifest submit`
چلانے سے پہلے اسے فائل کی صورت میں پیش کریں۔ Você pode usar um pino de pin کرنا
چاہیں اور DNS کو ہاتھ نہ لگانا ہو تو alias فلیگز خالی چھوڑ دیں۔

## مرحلہ 5b — گورننس پروپوزل تیار کریں

ہر مینی فیسٹ کے ساتھ پارلیمنٹ کے لئے تیار پروپوزل ہونا چاہیے تاکہ کوئی بھی
Sora شہری خصوصی اسناد ادھار لئے بغیر تبدیلی پیش کر سکے۔ enviar/assinar مراحل کے
O que fazer:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` کینونیکل `RegisterPinManifest` ہدایت, resumo de pedaços,
پالیسی, اور alias hint کو محفوظ کرتا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ پورٹل کے
ساتھ منسلک کریں تاکہ نمائندے آرٹیفیکٹس دوبارہ build کیے بغیر payload کا فرق
دیکھ سکیں۔ Chave de autoridade Torii کو کبھی نہیں چھوتی, کوئی بھی
شہری لوکل طور پر پروپوزل ڈرافٹ کر سکتا ہے۔

## مرحلہ 6 — provas اور ٹیلیمیٹری کی تصدیق

fixação کے بعد درج ذیل verificação determinística مراحل چلائیں:

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
  `torii_sorafs_replication_sla_total{outcome="missed"}` میں anomalias چیک کریں۔
- `npm run probe:portal` چلائیں تاکہ Try-It پروکسی اور ریکارڈ شدہ لنکس کو
  O pino mais importante é o que você precisa saber
- [Publicação e monitoramento](./publishing-monitoring.md) میں بیان کردہ مانیٹرنگ
  شواہد حاصل کریں تاکہ DOCS-3c کا observabilidade گیٹ اشاعت کے مراحل کے ساتھ
  پورا ہو۔ ہیلپر اب متعدد entradas `bindings` (سائٹ, OpenAPI, پورٹل SBOM, OpenAPI
  SBOM) قبول کرتا ہے اور اختیاری `hostname` گارڈ کے ذریعے ہدف host پر
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` Cartão de crédito Invocação de نیچے والی
  O JSON é um pacote de evidências (`portal.json`, `tryit.json`, `binding.json`,
  اور `checksums.sha256`) دونوں ریلیز ڈائریکٹری میں لکھتی ہے:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6a — gateway سرٹیفکیٹس کی منصوبہ بندی

TLS SAN/challenge GAR پیکٹس بنانے سے پہلے نکالیں تاکہ gateway ٹیم اور DNS
منظور کنندگان ایک ہی شواہد دیکھیں۔ O modelo DG-3 é um espelho com espelho
ہے, hosts curinga canônicos, SANs de host bonito, DNS-01 لیبلز, اور تجویز کردہ
Desafios ACME:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON کو ریلیز بنڈل کے ساتھ کمٹ کریں (یا چینج ٹکٹ کے ساتھ اپ لوڈ کریں) تاکہ
آپریٹرز Torii کی `torii.sorafs_gateway.acme` کنفیگ میں SAN ویلیوز چسپاں کر سکیں
اور GAR ریویورز mapeamentos canônicos/bonitos e derivações de host دوبارہ چلائے بغیر
تصدیق کر سکیں۔ اسی ریلیز میں پروموٹ ہونے والے ہر sufixo کے لئے اضافی `--name`
دلائل شامل کریں۔

## مرحلہ 6b — mapeamentos de host canônicos اخذ کریں

Cargas úteis GAR کے سانچوں سے پہلے ہر alias کے لئے mapeamento determinístico de host ریکارڈ
کریں۔ `cargo xtask soradns-hosts` ہر `--name` کو اس کے idioma canônico
(`<base32>.gw.sora.id`) میں ہیش کرتا ہے، مطلوبہ curinga (`*.gw.sora.id`) نکالتا
ہے، اور pretty host (`<alias>.gw.sora.name`) اخذ کرتا ہے۔ ریلیز آرٹیفیکٹس میں
آؤٹ پٹ محفوظ کریں تاکہ DG-3 ریویورز Submissão GAR کے ساتھ mapeamento کا فرق دیکھ
Dica:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب کوئی GAR یا ligação de gateway JSON مطلوبہ hosts میں سے کسی کو چھوڑ دے تو فوری
ناکامی کے لئے `--verify-host-patterns <file>` استعمال کریں۔ ہیلپر متعدد
verificação فائلیں قبول کرتا ہے, جس سے ایک ہی invocação میں modelo GAR اور
grampeado `portal.gateway.binding.json` دونوں کو lint کرنا آسان ہوتا ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

DNS/gateway چینج ٹکٹ کے ساتھ resumo JSON اور verificação لاگ منسلک کریں تاکہ
آڈیٹرز canônico, curinga, e hosts bonitos کی تصدیق derivações de host دوبارہ
چلائے بغیر کر سکیں۔ جب بھی بنڈل میں نئے aliases شامل ہوں تو یہ کمانڈ دوبارہ
چلائیں تاکہ بعد کے GAR اپڈیٹس میں وہی trilha de evidências برقرار رہے۔

## Passo 7 — Descritor de transição de DNS تیار کریں

cortes de produção کامیاب
envio (ligação de alias) کے بعد ہیلپر `artifacts/sorafs/portal.dns-cutover.json`
O que você precisa saber:- metadados de ligação de alias (namespace/nome/prova), resumo do manifesto, URL Torii,
  época submetida, autoridade)؛
- ریلیز سیاق و سباق (tag, rótulo de alias, caminhos de manifesto/CAR, plano de bloco, pacote Sigstore)؛
- ponteiros de verificação (sonda کمانڈ، alias + endpoint Torii)؛
- اختیاری controle de alterações فیلڈز (id do ticket, janela de transição, contato de operações),
  nome de host/zona de produção)؛
- `Sora-Route-Binding` grampeado ہیڈر سے اخذ کردہ metadados de promoção de rota
  (host canônico/CID, cabeçalho + caminhos de ligação, comandos de verificação), تاکہ GAR
  promoção e exercícios de fallback
- plano de rota gerado آرٹیفیکٹس (`gateway.route_plan.json`, modelos de cabeçalho,
  اور اختیاری rollback headers) تاکہ change tickets اور CI lint hooks ہر DG-3
  پیکٹ کے promoção/reversão canônica پلانز کا حوالہ منظوری سے پہلے دیکھ سکیں؛
- Eliminar metadados de invalidação de cache (endpoint de eliminação, variável de autenticação, carga útil JSON),
  Por exemplo, `curl` کمانڈ), Mais
- dicas de reversão no descritor (tag de liberação e resumo do manifesto) کی طرف
  اشارہ کریں تاکہ alterar tickets میں caminho alternativo determinístico شامل ہو۔

جب ریلیز کو cache purge درکار ہو تو descritor de corte کے ساتھ ایک canônico
O que fazer:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Você pode usar o `portal.cache_plan.json` para DG-3 para obter mais informações.
Existem hosts/caminhos determinísticos (com dicas de autenticação) e `PURGE`
solicitações descritor کا اختیاری metadados de cache سیکشن براہ راست اس فائل
کا حوالہ دے سکتا ہے, جس سے controle de mudança ریویورز ٹھیک انہی endpoints پر متفق
رہیں جو cutover کے دوران flush کیے جاتے ہیں۔

ہر DG-3 پیکٹ کو promoção + lista de verificação de reversão بھی درکار ہوتی ہے۔ اسے
`cargo xtask soradns-route-plan` کے ذریعے بنائیں تاکہ controle de mudança ریویورز
ہر alias کے لئے preflight, cutover, اور rollback مراحل کا سراغ لگا سکیں:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` میں hosts canônicos/bonitos, verificação de integridade encenada
یاددہانیاں, ligação GAR اپڈیٹس, limpezas de cache, e ações de reversão محفوظ ہوتے ہیں۔
GAR/binding/cutover آرٹیفیکٹس کے ساتھ اسے بنڈل کریں تاکہ Ops ایک ہی com script
قدموں کی مشق اور منظوری دے سکیں۔

`scripts/generate-dns-cutover-plan.mjs` é um descritor que pode ser usado
`sorafs-pin-release.sh` é um produto de alta qualidade دستی طور پر دوبارہ بنانے
O que fazer:

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

pin helper چلانے سے پہلے metadados opcionais کو variáveis de ambiente کے ذریعے
Dica:

| Variável | Finalidade |
|----------|---------|
| `DNS_CHANGE_TICKET` | descritor میں محفوظ ہونے والا Ticket ID۔ |
| `DNS_CUTOVER_WINDOW` | Janela de transição ISO8601 (exemplo `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | nome do host de produção + zona autoritativa۔ |
| `DNS_OPS_CONTACT` | alias de plantão یا contato de escalonamento۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | descritor میں ریکارڈ ہونے والا cache purge endpoint۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | purge token رکھنے والا env var (padrão: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | metadados de reversão کے لئے سابقہ ​​descritor de corte کا caminho۔ |

Revisão de alteração de DNS کے ساتھ JSON منسلک کریں تاکہ resumos de manifestos de aprovadores, alias
ligações, para sondar کمانڈز کو CI لاگز کریدے بغیر verificar کر سکیں۔ Sinalizadores CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, اور `--previous-dns-plan` وہی substitui فراہم کرتے ہیں
Ajudante کو CI کے باہر چلایا جائے۔

## مرحلہ 8 — esqueleto do arquivo de zona do resolvedor بنائیں (اختیاری)

Janela de transição de produção معلوم ہو تو ریلیز اسکرپٹ Esqueleto de arquivo de zona SNS
snippet do resolvedor Como registrar registros DNS e metadados
variáveis de ambiente یا opções CLI کے ذریعے پاس کریں؛ descritor de transição auxiliar
O cartão de crédito `scripts/sns_zonefile_skeleton.py` é um cartão de crédito کم از کم ایک
A/AAAA/CNAME é um resumo GAR (ou seja, carga útil GAR کا BLAKE3-256) فراہم
کریں۔ A zona/nome do host é a zona/nome do host `--dns-zonefile-out`.
ajudante `artifacts/sns/zonefiles/<zone>/<hostname>.json` پر لکھتا ہے اور
`ops/soradns/static_zones.<hostname>.json` é o snippet do resolvedor que está disponível para download| Variável/sinalizador | Finalidade |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | تیار شدہ esqueleto do zonefile کا path۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | caminho do snippet do resolvedor (o padrão `ops/soradns/static_zones.<hostname>.json`)۔ |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | تیار شدہ ریکارڈز پر لاگو TTL (padrão: 600 segundos)۔ |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Endereços IPv4 (env separado por vírgula e sinalizador CLI repetível)۔ |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Endereços IPv6۔ |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | اختیاری Destino CNAME۔ |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pinos SHA-256 SPKI (base64)۔ |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | versão computada do zonefile لیبل کو override کریں۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | início da janela de transição کے بجائے `effective_at` timestamp (RFC3339) کو کریں۔ |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | metadados میں ریکارڈ substituição literal de prova کریں۔ |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | metadados میں ریکارڈ Substituição de CID کریں۔ |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelamento do guardião (suave, duro, descongelamento, monitoramento, emergência)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | congela کے لئے Referência do bilhete do guardião/conselho۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | descongelamento کے لئے RFC3339 timestamp۔ |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | اضافی congelar notas (env separado por vírgula e sinalizador repetível)۔ |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | carga útil GAR assinada کا BLAKE3-256 digest (hex)۔ ligações de gateway |

Fluxo de trabalho do GitHub Actions یہ ویلیوز segredos do repositório سے پڑھتا ہے تاکہ ہر
pino de produção خودکار طور پر zonefile درج ذیل segredos/valores
کنفیگر کریں (strings میں vários valores فیلڈز کے لئے separados por vírgula فہرستیں ہو سکتی ہیں):

| Segredo | Finalidade |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | helper کو دیا جانے والا nome de host/zona de produção۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | descritor میں محفوظ alias de plantão۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | شائع ہونے والے Registros IPv4/IPv6۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | اختیاری Destino CNAME۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | pinos base64 SPKI۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی Entradas TXT۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | esqueleto میں محفوظ congelar metadados۔ |
| `DOCS_SORAFS_GAR_DIGEST` | carga útil GAR assinada کا BLAKE3 digest۔ codificado em hexadecimal |

`.github/workflows/docs-portal-sorafs-pin.yml` e `dns_change_ticket`
Entradas `dns_cutover_window` فراہم کریں تاکہ descritor/arquivo de zona درست janela de alteração
metadados انہیں خالی صرف ensaios a seco کے لئے چھوڑیں۔

Invocação de عام (runbook do proprietário SN-7 کے مطابق):

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

ajudante خودکار طور پر alterar ticket کو entrada TXT کے طور پر لے جاتا ہے اور cutover
início da janela کو `effective_at` timestamp کے طور پر herda کرتا ہے جب تک اسے
substituir نہ کیا جائے۔ O fluxo de trabalho mais importante
`docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### پبلک DNS ڈیلیگیشن نوٹ

esqueleto do zonefile صرف زون کے autoritativo عام انٹرنیٹ کو
نام سرورز ملنے کے لئے zona pai کی Delegação NS/DS رجسٹرار یا DNS فراہم کنندہ
پر الگ سے سیٹ کرنا ضروری ہے۔
- cutover apex/TLD کے لئے ALIAS/ANAME (específico do provedor) استعمال کریں یا gateway
  Quais IPs anycast são IPs Anycast e IPs A/AAAA são válidos
- سب ڈومینز کے لئے host bonito derivado (`<fqdn>.gw.sora.name`) کی طرف CNAME
  شائع کریں۔
- gateway de host canônico (`<hash>.gw.sora.id`)
  پبلک زون میں شائع نہیں ہوتا۔

### Gateway ہیڈر ٹیمپلیٹ

implantar auxiliar `portal.gateway.headers.txt` e `portal.gateway.binding.json` بھی
بناتا ہے، دو آرٹیفیکٹس جو DG-3 کی requisito de ligação de conteúdo do gateway پوری کرتے ہیں:

- `portal.gateway.headers.txt` مکمل Bloco de cabeçalho HTTP رکھتا ہے (جس میں
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, etc.
  Descritor `Sora-Route-Binding` شامل ہیں) جسے edge gateways ہر resposta کے
  ساتھ کرتے ہیں۔
- `portal.gateway.binding.json` وہی معلومات legível por máquina شکل میں ریکارڈ کرتا
  ہے تاکہ change tickets اور automação host/cid bindings کو shell آؤٹ پٹ کریدے
  بغیر diff کر سکیں۔

یہ خودکار طور پر `cargo xtask soradns-binding-template` کے ذریعے جنریٹ ہوتے ہیں اور alias, manifest digest, اور gateway
nome de host کو محفوظ کرتے ہیں جو `sorafs-pin-release.sh` کو فراہم کیے گئے تھے۔
bloco de cabeçalho دوبارہ بنانے یا personalizar کرنے کے لئے چلائیں:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`, `--permissions-template`, یا `--hsts-template` کے ذریعے padrão
modelos de cabeçalho کو override کریں جب کسی مخصوص implantação کو اضافی diretivas درکار ہوں؛
Os switches `--no-*` são os switches que possuem o cabeçalho do cabeçalho e o cabeçalho do arquivo

snippet de cabeçalho e solicitação de alteração de CDN کے ساتھ منسلک کریں اور JSON
pipeline de automação de gateway میں feed کریں تاکہ اصل promoção de host ریلیز evidência
سے میل کھائے۔Ajudante xtask اب canonical راستہ ہے۔ ریلیز اسکرپٹ auxiliar de verificação خودکار طور
پر چلاتا ہے تاکہ DG-3 ٹکٹس میں ہمیشہ تازہ evidência شامل ہو۔ جب بھی آپ vinculação JSON
O que você precisa saber sobre o seu cartão de crédito:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

یہ کمانڈ carga útil grampeada `Sora-Proof` کو ڈی کوڈ کرتی ہے, `Sora-Route-Binding`
metadados کو manifesto CID + nome do host کے ساتھ میچ کرتی ہے، اور اگر کوئی desvio de cabeçalho
ہو تو فوراً falhar کرتی ہے۔ جب بھی آپ یہ کمانڈ CI کے باہر چلائیں تو کنسول آؤٹ پٹ کو
Implementação de implementação
کہ ligação obrigatória سے پہلے validar ہوا تھا۔

> **Integração do descritor DNS:** `portal.dns-cutover.json` e `gateway_binding`
> سیکشن شامل کرتا ہے جو ان آرٹیفیکٹس (caminhos, CID de conteúdo, status de prova, اور
> modelo de cabeçalho literal) کی طرف اشارہ کرتا ہے **اور** estrofe `route_plan`
> `gateway.route_plan.json` para modelos de cabeçalho principal + rollback
> ہر DG-3 change ticket میں یہ بلاکس شامل کریں تاکہ ریویورز عین
> `Sora-Name`/`Sora-Proof`/`CSP` ویلیوز کا فرق دیکھ سکیں اور تصدیق کر سکیں کہ
> promoção/reversão de rota پلانز pacote de evidências کے ساتھ مطابقت رکھتے ہیں بغیر
> arquivo de compilação کھولے۔

## مرحلہ 9 — Monitores de publicação چلائیں

روڈ میپ ٹاسک **DOCS-3c** کے لئے مسلسل evidências درکار ہے کہ پورٹل، Experimente پروکسی،
Para ligações de gateway Março 7-8 meses consolidado
monitorar چلائیں اور اسے sondas programadas میں fio کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- Configuração `scripts/monitor-publishing.mjs` فائل لوڈ کرتا ہے (esquema کے لئے
  `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) e verificações
  Importante: sondagens de caminho do portal + validação de política de permissões/CSP, experimente proxy
  probes (sob o ponto de extremidade `/metrics`) e o verificador de ligação de gateway
  (`cargo xtask soradns-verify-binding`) ou verificações de alias/manifesto
  Sora-Content-CID کی موجودگی + متوقع ویلیو نافذ کرتا ہے۔
- اگر کوئی بھی probe ناکام ہو تو کمانڈ diferente de zero سے نکلتی ہے تاکہ CI, cron jobs,
  یا aliases de operadores de runbook
- `--json-out` ایک واحد resumo da carga útil JSON لکھتا ہے جس میں فی status de destino ہے؛
  `--evidence-dir` `summary.json`, `portal.json`, `tryit.json`, `binding.json`, اور
  `checksums.sha256` جاری کرتا ہے تاکہ گورننس ریویورز بغیر monitores دوبارہ چلائے
  Qual é a diferença entre as diferenças O cartão de crédito `artifacts/sorafs/<tag>/monitoring/` é compatível
  Pacote Sigstore e descritor de transição de DNS
- monitor آؤٹ پٹ, exportação Grafana (`dashboards/grafana/docs_portal.json`), اور
  ID de broca do Alertmanager
  آڈٹ ہو سکے۔ manual de monitor de publicação dedicado یہاں ہے:
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

Portal investiga HTTPS کا تقاضا کرتے ہیں اور `http://` بیس URLs کو مسترد کرتے ہیں جب
تک configuração do monitor میں `allowInsecureHttp` سیٹ نہ ہو؛ metas de produção/preparação
کو TLS پرکھیں اور یہ substituir صرف لوکل visualizações کے لئے فعال کریں۔

پورٹل کے لائیو ہونے کے بعد `npm run monitor:publishing` کو Buildkite/cron میں
automatizar کریں۔ یہی کمانڈ, جب URLs de produção پر apontados ہو، وہ صحت مند چیکس فراہم
کرتی ہے جن پر SRE/Docs ریلیز کے درمیان انحصار کرتے ہیں۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` formato 2-6 para encapsular cartão یہ:

1. `build/` کو tarball determinístico میں آرکائیو کرتا ہے،
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   Para `proof verify` چلاتا ہے,
3. Credenciais Torii
   چلاتا ہے، اور
4. `artifacts/sorafs/portal.pin.report.json`, اختیاری `portal.pin.proposal.json`,
  Descritor de transição de DNS (envios) e pacote de ligação de gateway
  (`portal.gateway.binding.json` + bloco de cabeçalho de texto) لکھتا ہے تاکہ governança,
  rede, operações de operações ٹیمیں CI لاگز کھنگالے بغیر pacote de evidências کا فرق دیکھ سکیں۔

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, اور (اختیاری)
`PIN_ALIAS_PROOF_PATH` سیٹ کریں۔ funcionamento a seco کے لئے `--skip-submit` استعمال کریں؛
Fluxo de trabalho do GitHub نیچے بیان کردہ `perform_submit` input

## مرحلہ 8 — Especificações OpenAPI e pacotes SBOM شائع کریں

DOCS-7 تقاضہ کرتا ہے کہ پورٹل build, especificação OpenAPI, اور SBOM آرٹیفیکٹس ایک ہی
pipeline determinístico موجودہ ajudantes تینوں کو کور کرتے ہیں:

1. **spec دوبارہ بنائیں اور سائن کریں۔**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```Você pode obter um instantâneo instantâneo com um valor de `--version=<label>`
   O cartão de crédito (مثلاً `2025-q3`)۔ instantâneo auxiliar کو
   `static/openapi/versions/<label>/torii.json` میں لکھتا ہے, اسے `versions/current`
   میں espelho کرتا ہے، اور metadados (SHA-256, status do manifesto, carimbo de data/hora atualizado)
   `static/openapi/versions.json` میں ریکارڈ کرتا ہے۔ ڈیولپر پورٹل یہ índice پڑھتا
   ہے تاکہ Swagger/RapiDoc پینلز seletor de versão دکھا سکیں اور متعلقہ digest/signature
   informações inline `--version` چھوڑ دینے سے پچھلے ریلیز لیبل برقرار رہتے
   Ponteiros `current` + `latest` تازہ ہوتے ہیں۔

   manifest SHA-256/BLAKE3 digests محفوظ کرتا ہے تاکہ gateway `/reference/torii-swagger`
   کے لئے `Sora-Proof` cabeçalhos grampeados کر سکے۔

2. **CycloneDX SBOMs خارج کریں۔** ریلیز pipeline پہلے ہی SBOMs baseados em syft کی توقع
   رکھتی ہے، جیسا کہ `docs/source/sorafs_release_pipeline_plan.md` میں درج ہے۔
   O que fazer para construir artefatos é o seguinte:

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

   Você pode usar o `manifest build` / `manifest sign` para obter mais informações
   ہر اثاثے کے لئے aliases ٹیون کریں (مثلاً spec کے لئے `docs-openapi.sora` اور
   سائن شدہ SBOM بنڈل کے لئے `docs-sbom.sora`)۔ Aliases رکھنے سے provas SoraDNS,
   GARs, e rollback ٹکٹس مخصوص payload تک محدود رہتے ہیں۔

4. **جمع کریں اور bind کریں۔** وہی autoridade + pacote Sigstore دوبارہ استعمال کریں،
   مگر ریلیز lista de verificação میں tupla de alias ریکارڈ کریں تاکہ آڈیٹرز دیکھ سکیں کہ کون سا
   Sora نام کس resumo do manifesto سے منسلک ہے۔

Manifestos Spec/SBOM کو پورٹل build کے ساتھ آرکائیو کرنا یقینی بناتا ہے کہ ہر
ریلیز ٹکٹ میں مکمل آرٹیفیکٹ سیٹ موجود ہو بغیر empacotador دوبارہ چلائے۔

### آٹومیشن ہیلپر (CI/script de pacote)

`./ci/package_docs_portal_sorafs.sh` مراحل 1-8 کو codificar کرتا ہے تاکہ روڈ میپ
آئٹم **DOCS-7** کو ایک کمانڈ میں چلایا جا سکے۔ ہیلپر:

- مطلوبہ پورٹل prep چلاتا ہے (`npm ci`, OpenAPI/sincronização norito, testes de widget)؛
- پورٹل, OpenAPI, اور SBOM CARs + pares de manifesto کو `sorafs_cli` کے ذریعے بناتا ہے؛
- Assinatura `sorafs_cli proof verify` (`--proof`) e Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`) چلاتا ہے؛
- تمام آرٹیفیکٹس کو `artifacts/devportal/sorafs/<timestamp>/` کے تحت چھوڑتا ہے اور
  `package_summary.json` Ferramentas de CI/liberação para ingerir ferramentas Mais
- `artifacts/devportal/sorafs/latest` کو ریفریش کر کے تازہ ترین رن کی طرف اشارہ کرتا ہے۔

Exemplo (Sigstore + PoR no pipeline do pipeline):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Bandeiras de قابلِ توجہ:

- `--out <dir>` – substituição da raiz do artefato کریں (data e hora padrão فولڈرز رکھتا ہے)۔
- `--skip-build` – موجودہ `docs/portal/build` دوبارہ استعمال کریں (جب CI آف لائن
  espelhos کی وجہ سے reconstruir نہ کر سکے)۔
- `--skip-sync-openapi` – `npm run sync-openapi` چھوڑ دیں جب `cargo xtask openapi`
  crates.io
- `--skip-sbom` – جب `syft` بائنری انسٹال نہ ہو تو اسے نہ چلائیں (اسکرپٹ warning دیتا ہے)۔
- `--proof` – ہر CAR/manifesto جوڑی کے لئے `sorafs_cli proof verify` چلائیں۔ vários arquivos
  cargas úteis CLI میں chunk-plan سپورٹ مانگتے ہیں, اس لئے اگر `plan chunk count`
  erros آئیں تو یہ فلیگ بند رکھیں اور upstream gate آنے پر دستی تصدیق کریں۔
- `--sign` – `sorafs_cli manifest sign` چلائیں۔ ٹوکن `SIGSTORE_ID_TOKEN` سے دیں
  (`--sigstore-token-env`) ou CLI ou `--sigstore-provider/--sigstore-audience`
  کے ذریعے اسے buscar کرنے دیں۔

artefatos de produção کے لئے `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
یہ اب پورٹل, OpenAPI, e cargas úteis SBOM پیک کرتا ہے, ہر manifesto پر سائن کرتا ہے،
اور `portal.additional_assets.json` میں اضافی اثاثوں کا metadados ریکارڈ کرتا ہے۔
ajudante وہی اختیاری botões سمجھتا ہے جو CI packager استعمال کرتا ہے، ساتھ ہی نئے
`--openapi-*`, `--portal-sbom-*`, e `--openapi-sbom-*` switches بھی, تاکہ آپ
ہر اثاثے کے لئے alias tuplas مقرر کر سکیں, `--openapi-sbom-source` کے ذریعے SBOM
A substituição de uma carga útil, a carga útil de uma carga útil (`--skip-openapi`/`--skip-sbom`),
Não padrão `syft` بائنری کی طرف `--syft-bin` سے اشارہ کر سکیں۔

یہ اسکرپٹ ہر کمانڈ دکھاتا ہے جو وہ چلاتا ہے؛ log کو `package_summary.json` کے ساتھ
ریلیز ٹکٹ میں شامل کریں تاکہ ریویورز CAR digests, planejar metadados, اور Sigstore pacote
hashes کا فرق shell ad-hoc آؤٹ پٹ دیکھے بغیر نکال سکیں۔

## Passo 9 — Gateway + SoraDNS کی تصدیق

cutover کا اعلان کرنے سے پہلے ثابت کریں کہ نیا alias SoraDNS کے ذریعے resolver ہو
رہا ہے اور gateways تازہ provas grampeadas کر رہے ہیں:

1. **porta de sonda چلائیں۔** `ci/check_sorafs_gateway_probe.sh` ڈیمو fixtures کے خلاف
   `cargo xtask sorafs-gateway-probe` چلاتا ہے جو
   `fixtures/sorafs_gateway/probe_demo/` میں موجود ہیں۔ حقیقی implantações کے لئے
   sondar o nome do host پر چلائیں:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```sonda `Sora-Name`, `Sora-Proof`, e `Sora-Proof-Status`
   `docs/source/sorafs_alias_policy.md` کے مطابق decodificar کرتا ہے اور اگر resumo do manifesto,
   TTLs, یا ligações GAR میں drift ہو تو ناکام ہو جاتا ہے۔

   Para verificações pontuais leves (por exemplo, quando apenas o pacote de ligação
   alterado), execute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   O auxiliar valida o pacote de ligação capturado e é útil para liberação
   tickets que precisam apenas de confirmação de vinculação em vez de um exercício de sondagem completo.

2. **evidência de perfuração جمع کریں۔** آپریٹر exercícios یا simulações do PagerDuty کے لئے sonda کو
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- …`
   کے ساتھ embrulho کریں۔ cabeçalhos/logs do wrapper
   `artifacts/sorafs_gateway_probe/<stamp>/` میں رکھتا ہے, `ops/drill-log.md` اپ ڈیٹ
   کرتا ہے، اور (اختیاری) ganchos de reversão یا Cargas úteis do PagerDuty بھی چلاتا ہے۔ SoraDNS
   Código de hardware IP `--host docs.sora` دیں تاکہ Código rígido IP نہ ہو۔

3. **ligações DNS کی تصدیق کریں۔** جب گورننس prova de alias شائع کرے تو probe میں
   حوالہ دیا گیا GAR فائل (`--gar`) ریکارڈ کریں اور ریلیز evidência کے ساتھ منسلک کریں۔
   O Resolver é um arquivo `tools/soradns-resolver` que pode ser usado para resolver problemas
   کہ entradas em cache نئے manifesto کو honra کریں۔ JSON منسلک کرنے سے پہلے چلائیں
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Mapeamento de host determinístico, metadados de manifesto, rótulos de telemetria offline
   validar ajudante assinado GAR کے ساتھ `--json-out` resumo بھی نکال سکتا ہے
   Revisores تاکہ بغیر binário کھولے قابلِ تصدیق evidência حاصل کریں۔
  نیا GAR بناتے وقت ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` پر صرف تب جائیں جب manifest فائل دستیاب نہ ہو)۔ ajudante
  CID **اور** BLAKE3 digest دونوں manifesto JSON سے براہ راست نکالتا ہے، espaço em branco
  trim کرتا ہے، دہرے `--telemetry-label` sinalizadores ختم کرتا ہے, classificação de rótulos کرتا ہے, اور
  JSON é um modelo padrão de CSP/HSTS/Permissions-Policy.
  carga útil determinística رہے، چاہے آپریٹرز rótulos مختلف shells سے اکٹھے کریں۔

4. **métricas de alias پر نظر رکھیں۔** `torii_sorafs_alias_cache_refresh_duration_ms`
   اور `torii_sorafs_gateway_refusals_total{profile="docs"}` کو sonda کے دوران
   اسکرین پر رکھیں؛ Série de jogos `dashboards/grafana/docs_portal.json` میں چارٹ ہیں۔

## Etapa 10 — Monitoramento e agrupamento de evidências

- **Dashboards۔** ہر ریلیز کے لئے `dashboards/grafana/docs_portal.json` (SLOs do portal)،
  `dashboards/grafana/sorafs_gateway_observability.json` (latência do gateway + prova de integridade) ،
  Sobre `dashboards/grafana/sorafs_fetch_observability.json` (saúde do orquestrador)
  exportar کریں۔ Exportações JSON
  Consultas Prometheus
- **Arquivos de sonda۔** `artifacts/sorafs_gateway_probe/<stamp>/` کو git-annex یا
  اپنے balde de evidências میں رکھیں۔ resumo da sonda, cabeçalhos e script de telemetria
  Carga útil do PagerDuty
- **Pacote de lançamento۔** پورٹل/SBOM/OpenAPI Resumos do CAR, pacotes de manifesto, Sigstore
  assinaturas, `portal.pin.report.json`, logs de sonda Try-It, e relatórios de verificação de link e
  Um carimbo de data/hora (por `artifacts/sorafs/devportal/20260212T1103Z/`) é um registro de data e hora
- **Registro de perfuração۔** جب sondas کسی broca کا حصہ ہوں تو
  `scripts/telemetry/run_sorafs_gateway_probe.sh` کو `ops/drill-log.md` میں anexar
  کرنے دیں تاکہ وہی evidência requisito de caos SNNet-5 پوری کرے۔
- **Links de ingresso۔** alterar ticket میں IDs do painel Grafana یا exportações PNG anexadas کا
  حوالہ دیں, ساتھ ہی caminho do relatório do probe بھی شامل کریں, تاکہ acesso ao shell dos revisores
  Verificação cruzada de SLOs

## Etapa 11 — exercício de busca de múltiplas fontes e evidência de placar

SoraFS پر اشاعت اب evidência de busca de múltiplas fontes (DOCS-7/SF-6) کا تقاضہ کرتی ہے،
اور یہ اوپر والے Provas de DNS/gateway pino de manifesto کرنے کے بعد:

1. **live manifest کے خلاف `sorafs_fetch` چلائیں۔** مراحل 2-3 کے plano/manifesto
   آرٹیفیکٹس اور ہر provedor کے جاری کردہ credenciais de gateway استعمال کریں۔ ہر
   آؤٹ پٹ محفوظ کریں تاکہ آڈیٹرز orquestrador کے trilha de decisão کو replay کر سکیں:

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

   - manifesto میں حوالہ دی گئی anúncios de provedor پہلے حاصل کریں (مثلاً
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور انہیں `--provider-advert name=path` کے ذریعے پاس کریں تاکہ placar
     capacidade determinística do Windows avaliar کر سکے۔
     `--allow-implicit-provider-metadata` **صرف** CI میں replay de jogos کرتے وقت
     استعمال کریں؛ brocas de produção میں pino کے ساتھ آنے والے anúncios assinados ہی دیں۔
   - جب manifesto اضافی regiões کی طرف اشارہ کرے تو متعلقہ tuplas de provedor کے ساتھ
     کمانڈ دہرائیں تاکہ ہر cache/alias کی artefato de busca correspondente ہو۔

2. **آؤٹ پٹس آرکائیو کریں۔** `scoreboard.json`, `providers.ndjson`, `fetch.json`, اور
   `chunk_receipts.ndjson` کو ریلیز evidência فولڈر کے اندر رکھیں۔ یہ فائلیں par
   ponderação, orçamento de nova tentativa, latência EWMA, e receitas por bloco محفوظ کرتی ہیں
   جنہیں گورننس پیکٹ SF-7 کے لئے برقرار رکھے۔3. **ٹیلیمیٹری اپ ڈیٹ کریں۔** saídas de busca e **SoraFS Observabilidade de busca**
   ڈیش بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) میں امپورٹ کریں،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` e painéis de faixa de provedor
   anomalias Instantâneos do painel Grafana کو ریلیز ٹکٹ میں caminho do placar کے ساتھ
   لنک کریں۔

4. **regras de alerta کا fumaça کریں۔** `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   چلا کر ریلیز بند کرنے سے پہلے Pacote de alerta Prometheus validar کریں۔ ferramenta promocional
   آؤٹ پٹ کو ٹکٹ کے ساتھ منسلک کریں تاکہ DOCS-7 ریویورز تصدیق کر سکیں کہ stall اور
   alertas de provedor lento

5. **CI میں wire کریں۔** پورٹل pin workflow میں `sorafs_fetch` مرحلہ `perform_fetch_probe`
   entrada کے پیچھے رکھا گیا ہے؛ preparação/produção
   pacote de manifesto de evidências لوکل brocas وہی اسکرپٹ
   Você pode exportar tokens de gateway para `PIN_FETCH_PROVIDERS`
   lista de provedores separados por vírgula

## پروموشن, observabilidade, e reversão

1. **پروموشن:** preparação e produção کے لئے علیحدہ aliases رکھیں۔ پروموشن کے لئے
   O manifesto/pacote é o `manifest submit` que está disponível
   `--alias-namespace/--alias-name` کو alias de produção پر سوئچ کریں۔ Como obter um controle de qualidade
   شدہ pino de teste کے بعد دوبارہ construir یا assinar novamente کی ضرورت نہیں رہتی۔
2. **Monitoramento:** registro de pinos ڈیش بورڈ
   (`docs/source/grafana_sorafs_pin_registry.json`) Como testar sondas
   (`docs/portal/docs/devportal/observability.md` دیکھیں) امپورٹ کریں۔ desvio de soma de verificação,
   sondagens com falha, picos de repetição de prova e alerta de alerta
3. **Reversão:** واپس جانے کے لئے پچھلا manifest دوبارہ submit کریں (یا موجودہ alias
   aposentar کریں) `sorafs_cli manifest submit --alias ... --retire` کے ذریعے۔ ہمیشہ
   último pacote em bom estado اور Resumo do CAR محفوظ رکھیں تاکہ CI لاگز girar ہو جائیں تو
   provas de reversão

## Modelo de fluxo de trabalho de CI

O que você precisa saber sobre o pipeline de pipeline:

1. Build + lint (`npm ci`, `npm run build`, geração de soma de verificação).
2. Pacote (`car pack`) e manifestos computacionais کریں۔
3. OIDC com escopo de trabalho é o sinal کریں (`manifest sign`).
4. Auditoria de artefatos اپ لوڈ کریں (CAR, manifesto, pacote, plano, resumos).
5. registro de pin میں enviar کریں:
   - Solicitações pull → `docs-preview.sora`.
   - Tags/ramos protegidos → promoção de alias de produção.
6. saída سے پہلے sondas + portões de verificação de prova چلائیں۔

Liberações manuais `.github/workflows/docs-portal-sorafs-pin.yml` کے لئے یہ تمام
مراحل جوڑتا ہے۔ fluxo de trabalho:

- Construir/testar کرتا ہے،
- `scripts/sorafs-pin-release.sh` کے ذریعے build پیکج کرتا ہے،
- GitHub OIDC استعمال کر کے manifest bundle سائن/verify کرتا ہے،
- CAR/manifesto/pacote/plano/resumos de prova کو artefatos کے طور پر اپ لوڈ کرتا ہے، اور
- (اختیاری) segredos موجود ہوں تو manifesto + vinculação de alias enviar کرتا ہے۔

trabalho چلانے سے پہلے درج ذیل segredos/variáveis do repositório کنفیگر کریں:

| Nome | Finalidade |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Host Torii ou `/v1/sorafs/pin/register` é um host Torii ou `/v1/sorafs/pin/register` |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | submissões کے ساتھ ریکارڈ ہونے والا identificador de época۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | envio de manifesto کے لئے autoridade de assinatura۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | tupla de alias جو `perform_submit` true ہونے پر manifest سے bind ہوتا ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Pacote de prova de alias codificado em Base64 (اختیاری؛ ligação de alias چھوڑنے کے لئے omitir کریں)۔ |
| `DOCS_ANALYTICS_*` | موجودہ endpoints de análise/sondagem جو دوسرے fluxos de trabalho میں reutilização ہوتے ہیں۔ |

UI de ações e gatilho de fluxo de trabalho کریں:

1. `alias_label` فراہم کریں (مثلاً `docs.sora.link`), opcional `proposal_alias`,
   Sobreposição opcional `release_tag`۔
2. artefatos بنانے کے لئے `perform_submit` کو چھوڑیں desmarcado (Torii کو touch کیے بغیر)
   یا اسے enable کریں تاکہ alias configurado پر براہ راست publicar ہو جائے۔

`docs/source/sorafs_ci_templates.md` é um repositório de repositório de banco de dados
auxiliares de CI genéricos

## Lista de verificação

- [] `npm run build`, `npm run test:*`, ou `npm run check:links` گرین ہیں۔
- [ ] `build/checksums.sha256` e `build/release.json` artefatos میں محفوظ ہیں۔
- [ ] CAR, plano, manifesto, اور resumo `artifacts/` کے تحت بنے ہیں۔
- [ ] Pacote Sigstore + assinatura separada لاگز کے ساتھ محفوظ ہیں۔
- [ ] `portal.manifest.submit.summary.json` ou `portal.manifest.submit.response.json`
      submissões کے وقت محفوظ ہوئے۔
- [] `portal.pin.report.json` (`portal.pin.proposal.json`)
      Artefatos CAR/manifesto کے ساتھ آرکائیو ہوئے۔
- [ ] `proof verify` e `manifest verify-signature` لاگز آرکائیو ہوئے۔
- [] Painéis Grafana اپ ڈیٹ ہیں + Sondas Try-It کامیاب ہیں۔
- [] notas de reversão (ID do manifesto anterior + resumo do alias)