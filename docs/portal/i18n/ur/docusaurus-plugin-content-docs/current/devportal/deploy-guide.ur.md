---
lang: ur
direction: rtl
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
alias پروموشن، توثیق، اور rollback ڈرلز کو کور کرتی ہے تاکہ ہر preview اور
release آرٹیفیکٹ قابلِ اعادہ اور قابلِ آڈٹ ہو۔

یہ فلو فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری (`--features cli` کے ساتھ
build شدہ) ہے، pin-registry اجازتوں والے Torii endpoint تک رسائی ہے، اور
Sigstore کے لئے OIDC اسناد ہیں۔ طویل مدتی راز (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii ٹوکنز) کو اپنے CI والٹ میں رکھیں؛ لوکل رنز انہیں
shell exports سے لوڈ کر سکتے ہیں۔

## پیشگی شرائط

- Node 18.18+ کے ساتھ `npm` یا `pnpm`.
- `sorafs_cli` جو `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے حاصل ہو۔
- Torii URL جو `/v1/sorafs/*` ظاہر کرے اور ایک اتھارٹی اکاؤنٹ/پرائیویٹ کی جو
  مینی فیسٹس اور aliases جمع کر سکے۔
- OIDC issuer (GitHub Actions, GitLab, workload identity وغیرہ) تاکہ
  `SIGSTORE_ID_TOKEN` منٹ کیا جا سکے۔
- اختیاری: dry runs کے لئے `examples/sorafs_cli_quickstart.sh` اور GitHub/GitLab
  workflows کے لئے `docs/source/sorafs_ci_templates.md`.
- Try it OAuth ویری ایبلز (`DOCS_OAUTH_*`) کنفیگر کریں اور build کو لیب سے باہر
  پروموٹ کرنے سے پہلے [security-hardening checklist](./security-hardening.md)
  چلائیں۔ اب اگر یہ ویری ایبلز موجود نہ ہوں یا TTL/polling knobs نافذ شدہ
  حدود سے باہر ہوں تو پورٹل build ناکام ہو جاتا ہے؛ `DOCS_OAUTH_ALLOW_INSECURE=1`
  صرف عارضی لوکل previews کے لئے export کریں۔ pen-test ثبوت ریلیز ٹکٹ کے ساتھ
  منسلک کریں۔

## مرحلہ 0 — Try it پروکسی بنڈل محفوظ کریں

Netlify یا gateway پر preview پروموٹ کرنے سے پہلے Try it پروکسی سورسز اور
سائن شدہ OpenAPI مینی فیسٹ digest کو ایک متعین بنڈل میں اسٹیمپ کریں:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` پروکسی/probe/rollback helpers کاپی کرتا ہے،
OpenAPI signature کی تصدیق کرتا ہے، اور `release.json` کے ساتھ
`checksums.sha256` لکھتا ہے۔ اس بنڈل کو Netlify/SoraFS gateway پروموشن ٹکٹ کے
ساتھ منسلک کریں تاکہ ریویورز عین وہی پروکسی سورسز اور Torii ٹارگٹ ہنٹس بغیر
دوبارہ build کے ری پلے کر سکیں۔ بنڈل یہ بھی ریکارڈ کرتا ہے کہ آیا client-supplied
bearers فعال تھے (`allow_client_auth`) تاکہ rollout پلان اور CSP قواعد ہم آہنگ
رہیں۔

## مرحلہ 1 — پورٹل build اور lint

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
  CAR/manifest میں پِن کیا جاتا ہے۔

دونوں فائلیں CAR سمری کے ساتھ آرکائیو کریں تاکہ ریویورز بغیر دوبارہ build کیے
preview آرٹیفیکٹس کا فرق دیکھ سکیں۔

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

سمری JSON chunk counts، digests، اور proof-planning hints محفوظ کرتا ہے جنہیں
`manifest build` اور CI ڈیش بورڈز بعد میں دوبارہ استعمال کرتے ہیں۔

## مرحلہ 2b — OpenAPI اور SBOM معاون پیکج کریں

DOCS-7 تقاضہ کرتا ہے کہ پورٹل سائٹ، OpenAPI اسنیپ شاٹ، اور SBOM payloads کو
الگ الگ مینی فیسٹس کے طور پر شائع کیا جائے تاکہ gateways ہر آرٹیفیکٹ کے لئے
`Sora-Proof`/`Sora-Content-CID` ہیڈرز اسٹپل کر سکیں۔ ریلیز ہیلپر
(`scripts/sorafs-pin-release.sh`) پہلے ہی OpenAPI ڈائریکٹری (`static/openapi/`)
اور `syft` سے بننے والے SBOMs کو علیحدہ `openapi.*`/`*-sbom.*` CARs میں پیک
کرتا ہے اور میٹا ڈیٹا `artifacts/sorafs/portal.additional_assets.json` میں
ریکارڈ کرتا ہے۔ دستی فلو چلاتے وقت، ہر payload کے لئے مراحل 2-4 دہرائیں اور
اس کے اپنے prefixes اور metadata labels استعمال کریں (مثال کے طور پر
`--car-out "$OUT"/openapi.car` کے ساتھ `--metadata alias_label=docs.sora.link/openapi`).
DNS سوئچ کرنے سے پہلے ہر manifest/alias جوڑے کو Torii میں رجسٹر کریں (سائٹ،
OpenAPI، پورٹل SBOM، OpenAPI SBOM) تاکہ gateway تمام شائع شدہ آرٹیفیکٹس کے لئے
stapled proofs فراہم کر سکے۔

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

pin-policy فلیگز کو اپنے ریلیز ونڈو کے مطابق ٹیون کریں (مثال کے طور پر
canaries کے لئے `--pin-storage-class hot`)۔ JSON ورژن اختیاری ہے مگر code review
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
```

bundle مینی فیسٹ digest، chunk digests، اور OIDC ٹوکن کا BLAKE3 hash ریکارڈ
کرتا ہے بغیر JWT محفوظ کیے۔ bundle اور detached signature دونوں سنبھال کر
رکھیں؛ production promotions انہی آرٹیفیکٹس کو دوبارہ سائن کیے بغیر استعمال
کر سکتی ہیں۔ لوکل رنز میں provider فلیگز کو `--identity-token-env` سے بدل سکتے
ہیں (یا ماحول میں `SIGSTORE_ID_TOKEN` سیٹ کر سکتے ہیں) جب کوئی بیرونی OIDC
ہیلپر ٹوکن جاری کرے۔

## مرحلہ 5 — pin رجسٹری میں جمع کرائیں

سائن شدہ مینی فیسٹ (اور chunk plan) Torii کو جمع کرائیں۔ ہمیشہ summary طلب کریں
تاکہ رجسٹری entry/alias proof قابلِ آڈٹ رہے۔

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

جب preview یا canary alias (`docs-preview.sora`) رول آؤٹ کریں تو منفرد alias کے
ساتھ submission دوبارہ کریں تاکہ QA production پروموشن سے پہلے مواد کی تصدیق کر سکے۔

Alias binding کے لئے تین فیلڈز درکار ہیں: `--alias-namespace`, `--alias-name`,
اور `--alias-proof`۔ گورننس alias درخواست منظور ہونے پر proof bundle (base64 یا
Norito bytes) تیار کرتی ہے؛ اسے CI secrets میں رکھیں اور `manifest submit`
چلانے سے پہلے اسے فائل کی صورت میں پیش کریں۔ جب آپ صرف مینی فیسٹ pin کرنا
چاہیں اور DNS کو ہاتھ نہ لگانا ہو تو alias فلیگز خالی چھوڑ دیں۔

## مرحلہ 5b — گورننس پروپوزل تیار کریں

ہر مینی فیسٹ کے ساتھ پارلیمنٹ کے لئے تیار پروپوزل ہونا چاہیے تاکہ کوئی بھی
Sora شہری خصوصی اسناد ادھار لئے بغیر تبدیلی پیش کر سکے۔ submit/sign مراحل کے
بعد چلائیں:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` کینونیکل `RegisterPinManifest` ہدایت، chunk digest،
پالیسی، اور alias hint کو محفوظ کرتا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ پورٹل کے
ساتھ منسلک کریں تاکہ نمائندے آرٹیفیکٹس دوبارہ build کیے بغیر payload کا فرق
دیکھ سکیں۔ چونکہ یہ کمانڈ Torii authority key کو کبھی نہیں چھوتی، کوئی بھی
شہری لوکل طور پر پروپوزل ڈرافٹ کر سکتا ہے۔

## مرحلہ 6 — proofs اور ٹیلیمیٹری کی تصدیق

pinning کے بعد درج ذیل deterministic verification مراحل چلائیں:

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

- `torii_sorafs_gateway_refusals_total` اور
  `torii_sorafs_replication_sla_total{outcome="missed"}` میں anomalies چیک کریں۔
- `npm run probe:portal` چلائیں تاکہ Try-It پروکسی اور ریکارڈ شدہ لنکس کو
  نئے pin شدہ مواد کے خلاف پرکھا جا سکے۔
- [Publishing & Monitoring](./publishing-monitoring.md) میں بیان کردہ مانیٹرنگ
  شواہد حاصل کریں تاکہ DOCS-3c کا observability گیٹ اشاعت کے مراحل کے ساتھ
  پورا ہو۔ ہیلپر اب متعدد `bindings` entries (سائٹ، OpenAPI، پورٹل SBOM، OpenAPI
  SBOM) قبول کرتا ہے اور اختیاری `hostname` گارڈ کے ذریعے ہدف host پر
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` نافذ کرتا ہے۔ نیچے والی invocation
  ایک JSON سمری اور evidence bundle (`portal.json`, `tryit.json`, `binding.json`,
  اور `checksums.sha256`) دونوں ریلیز ڈائریکٹری میں لکھتی ہے:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6a — gateway سرٹیفکیٹس کی منصوبہ بندی

TLS SAN/challenge پلان GAR پیکٹس بنانے سے پہلے نکالیں تاکہ gateway ٹیم اور DNS
منظور کنندگان ایک ہی شواہد دیکھیں۔ نیا ہیلپر DG-3 آٹومیشن ان پٹس کو mirror کرتا
ہے، canonical wildcard hosts، pretty-host SANs، DNS-01 لیبلز، اور تجویز کردہ
ACME challenges شمار کرتا ہے:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON کو ریلیز بنڈل کے ساتھ کمٹ کریں (یا چینج ٹکٹ کے ساتھ اپ لوڈ کریں) تاکہ
آپریٹرز Torii کی `torii.sorafs_gateway.acme` کنفیگ میں SAN ویلیوز چسپاں کر سکیں
اور GAR ریویورز canonical/pretty mappings کو host derivations دوبارہ چلائے بغیر
تصدیق کر سکیں۔ اسی ریلیز میں پروموٹ ہونے والے ہر suffix کے لئے اضافی `--name`
دلائل شامل کریں۔

## مرحلہ 6b — canonical host mappings اخذ کریں

GAR payloads کے سانچوں سے پہلے ہر alias کے لئے deterministic host mapping ریکارڈ
کریں۔ `cargo xtask soradns-hosts` ہر `--name` کو اس کے canonical لیبل
(`<base32>.gw.sora.id`) میں ہیش کرتا ہے، مطلوبہ wildcard (`*.gw.sora.id`) نکالتا
ہے، اور pretty host (`<alias>.gw.sora.name`) اخذ کرتا ہے۔ ریلیز آرٹیفیکٹس میں
آؤٹ پٹ محفوظ کریں تاکہ DG-3 ریویورز GAR submission کے ساتھ mapping کا فرق دیکھ
سکیں:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب کوئی GAR یا gateway binding JSON مطلوبہ hosts میں سے کسی کو چھوڑ دے تو فوری
ناکامی کے لئے `--verify-host-patterns <file>` استعمال کریں۔ ہیلپر متعدد
verification فائلیں قبول کرتا ہے، جس سے ایک ہی invocation میں GAR template اور
stapled `portal.gateway.binding.json` دونوں کو lint کرنا آسان ہوتا ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

DNS/gateway چینج ٹکٹ کے ساتھ summary JSON اور verification لاگ منسلک کریں تاکہ
آڈیٹرز canonical، wildcard، اور pretty hosts کی تصدیق host derivations دوبارہ
چلائے بغیر کر سکیں۔ جب بھی بنڈل میں نئے aliases شامل ہوں تو یہ کمانڈ دوبارہ
چلائیں تاکہ بعد کے GAR اپڈیٹس میں وہی evidence trail برقرار رہے۔

## مرحلہ 7 — DNS cutover descriptor تیار کریں

production cutovers کے لئے آڈیٹ کے قابل تبدیلی پیکٹ درکار ہوتا ہے۔ کامیاب
submission (alias binding) کے بعد ہیلپر `artifacts/sorafs/portal.dns-cutover.json`
بناتا ہے، جس میں یہ شامل ہے:

- alias binding metadata (namespace/name/proof، manifest digest، Torii URL،
  submitted epoch، authority)؛
- ریلیز سیاق و سباق (tag، alias label، manifest/CAR paths، chunk plan، Sigstore bundle)؛
- verification pointers (probe کمانڈ، alias + Torii endpoint)؛
- اختیاری change-control فیلڈز (ticket id، cutover window، ops contact،
  production hostname/zone)؛
- stapled `Sora-Route-Binding` ہیڈر سے اخذ کردہ route promotion metadata
  (canonical host/CID، header + binding paths، verification commands)، تاکہ GAR
  promotion اور fallback drills ایک ہی شواہد کا حوالہ دیں؛
- generated route-plan آرٹیفیکٹس (`gateway.route_plan.json`, header templates,
  اور اختیاری rollback headers) تاکہ change tickets اور CI lint hooks ہر DG-3
  پیکٹ کے canonical promotion/rollback پلانز کا حوالہ منظوری سے پہلے دیکھ سکیں؛
- اختیاری cache invalidation metadata (purge endpoint، auth variable، JSON payload،
  اور مثال `curl` کمانڈ)؛ اور
- rollback hints جو پچھلے descriptor (release tag اور manifest digest) کی طرف
  اشارہ کریں تاکہ change tickets میں deterministic fallback path شامل ہو۔

جب ریلیز کو cache purge درکار ہو تو cutover descriptor کے ساتھ ایک canonical
پلان بنائیں:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

حاصل شدہ `portal.cache_plan.json` کو DG-3 پیکٹ کے ساتھ منسلک کریں تاکہ آپریٹرز
کے پاس deterministic hosts/paths (اور ملتے جلتے auth hints) ہوں جب وہ `PURGE`
requests بھیجیں۔ descriptor کا اختیاری cache metadata سیکشن براہ راست اس فائل
کا حوالہ دے سکتا ہے، جس سے change-control ریویورز ٹھیک انہی endpoints پر متفق
رہیں جو cutover کے دوران flush کیے جاتے ہیں۔

ہر DG-3 پیکٹ کو promotion + rollback checklist بھی درکار ہوتی ہے۔ اسے
`cargo xtask soradns-route-plan` کے ذریعے بنائیں تاکہ change-control ریویورز
ہر alias کے لئے preflight، cutover، اور rollback مراحل کا سراغ لگا سکیں:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` میں canonical/pretty hosts، staged health-check
یاددہانیاں، GAR binding اپڈیٹس، cache purges، اور rollback actions محفوظ ہوتے ہیں۔
GAR/binding/cutover آرٹیفیکٹس کے ساتھ اسے بنڈل کریں تاکہ Ops ایک ہی scripted
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

pin helper چلانے سے پہلے optional metadata کو environment variables کے ذریعے
بھریں:

| Variable | Purpose |
|----------|---------|
| `DNS_CHANGE_TICKET` | descriptor میں محفوظ ہونے والا Ticket ID۔ |
| `DNS_CUTOVER_WINDOW` | ISO8601 cutover window (مثال `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | production hostname + authoritative zone۔ |
| `DNS_OPS_CONTACT` | on-call alias یا escalation contact۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | descriptor میں ریکارڈ ہونے والا cache purge endpoint۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | purge token رکھنے والا env var (default: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | rollback metadata کے لئے سابقہ cutover descriptor کا path۔ |

DNS change review کے ساتھ JSON منسلک کریں تاکہ approvers manifest digests، alias
bindings، اور probe کمانڈز کو CI لاگز کریدے بغیر verify کر سکیں۔ CLI flags
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, اور `--previous-dns-plan` وہی overrides فراہم کرتے ہیں
جب helper کو CI کے باہر چلایا جائے۔

## مرحلہ 8 — resolver zonefile skeleton بنائیں (اختیاری)

جب production cutover window معلوم ہو تو ریلیز اسکرپٹ SNS zonefile skeleton اور
resolver snippet خودکار طور پر بنا سکتا ہے۔ مطلوبہ DNS records اور metadata کو
environment variables یا CLI options کے ذریعے پاس کریں؛ helper cutover descriptor
بننے کے فوراً بعد `scripts/sns_zonefile_skeleton.py` چلائے گا۔ کم از کم ایک
A/AAAA/CNAME ویلیو اور GAR digest (سائن شدہ GAR payload کا BLAKE3-256) فراہم
کریں۔ اگر zone/hostname معلوم ہوں اور `--dns-zonefile-out` چھوڑ دیا جائے تو
helper `artifacts/sns/zonefiles/<zone>/<hostname>.json` پر لکھتا ہے اور
`ops/soradns/static_zones.<hostname>.json` کو resolver snippet کے طور پر بھرتا ہے۔

| Variable / flag | Purpose |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | تیار شدہ zonefile skeleton کا path۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | resolver snippet path (اگر چھوڑا جائے تو default `ops/soradns/static_zones.<hostname>.json`)۔ |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | تیار شدہ ریکارڈز پر لاگو TTL (default: 600 seconds)۔ |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 addresses (comma-separated env یا repeatable CLI flag)۔ |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 addresses۔ |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | اختیاری CNAME target۔ |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI pins (base64)۔ |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | اضافی TXT entries (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | computed zonefile version لیبل کو override کریں۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | cutover window start کے بجائے `effective_at` timestamp (RFC3339) کو forced کریں۔ |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | metadata میں ریکارڈ proof literal override کریں۔ |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | metadata میں ریکارڈ CID override کریں۔ |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian freeze state (soft, hard, thawing, monitoring, emergency)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | freezes کے لئے Guardian/council ticket reference۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | thawing کے لئے RFC3339 timestamp۔ |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | اضافی freeze notes (comma-separated env یا repeatable flag)۔ |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | signed GAR payload کا BLAKE3-256 digest (hex)۔ gateway bindings ہونے پر لازم۔ |

GitHub Actions workflow یہ ویلیوز repository secrets سے پڑھتا ہے تاکہ ہر
production pin خودکار طور پر zonefile آرٹیفیکٹس بنائے۔ درج ذیل secrets/values
کنفیگر کریں (strings میں multi-value فیلڈز کے لئے comma-separated فہرستیں ہو سکتی ہیں):

| Secret | Purpose |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | helper کو دیا جانے والا production hostname/zone۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | descriptor میں محفوظ on-call alias۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | شائع ہونے والے IPv4/IPv6 records۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | اختیاری CNAME target۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | base64 SPKI pins۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی TXT entries۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | skeleton میں محفوظ freeze metadata۔ |
| `DOCS_SORAFS_GAR_DIGEST` | signed GAR payload کا hex-encoded BLAKE3 digest۔ |

`.github/workflows/docs-portal-sorafs-pin.yml` چلاتے وقت `dns_change_ticket` اور
`dns_cutover_window` inputs فراہم کریں تاکہ descriptor/zonefile درست change window
metadata لے۔ انہیں خالی صرف dry runs کے لئے چھوڑیں۔

عام invocation (SN-7 owner runbook کے مطابق):

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

helper خودکار طور پر change ticket کو TXT entry کے طور پر لے جاتا ہے اور cutover
window start کو `effective_at` timestamp کے طور پر inherits کرتا ہے جب تک اسے
override نہ کیا جائے۔ مکمل عملی workflow کے لئے
`docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### پبلک DNS ڈیلیگیشن نوٹ

zonefile skeleton صرف زون کے authoritative ریکارڈز متعین کرتا ہے۔ عام انٹرنیٹ کو
نام سرورز ملنے کے لئے parent‑zone کی NS/DS delegation رجسٹرار یا DNS فراہم کنندہ
پر الگ سے سیٹ کرنا ضروری ہے۔
- apex/TLD cutover کے لئے ALIAS/ANAME (provider‑specific) استعمال کریں یا gateway
  کے anycast IPs کی طرف اشارہ کرنے والے A/AAAA ریکارڈز شائع کریں۔
- سب ڈومینز کے لئے derived pretty host (`<fqdn>.gw.sora.name`) کی طرف CNAME
  شائع کریں۔
- canonical host (`<hash>.gw.sora.id`) gateway ڈومین کے تحت رہتا ہے اور آپ کے
  پبلک زون میں شائع نہیں ہوتا۔

### Gateway ہیڈر ٹیمپلیٹ

deploy helper `portal.gateway.headers.txt` اور `portal.gateway.binding.json` بھی
بناتا ہے، دو آرٹیفیکٹس جو DG-3 کی gateway-content-binding requirement پوری کرتے ہیں:

- `portal.gateway.headers.txt` مکمل HTTP header block رکھتا ہے (جس میں
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS، اور
  `Sora-Route-Binding` descriptor شامل ہیں) جسے edge gateways ہر response کے
  ساتھ stapled کرتے ہیں۔
- `portal.gateway.binding.json` وہی معلومات machine-readable شکل میں ریکارڈ کرتا
  ہے تاکہ change tickets اور automation host/cid bindings کو shell آؤٹ پٹ کریدے
  بغیر diff کر سکیں۔

یہ خودکار طور پر `cargo xtask soradns-binding-template` کے ذریعے جنریٹ ہوتے ہیں اور alias، manifest digest، اور gateway
hostname کو محفوظ کرتے ہیں جو `sorafs-pin-release.sh` کو فراہم کیے گئے تھے۔
header block دوبارہ بنانے یا customize کرنے کے لئے چلائیں:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`, `--permissions-template`, یا `--hsts-template` کے ذریعے default
header templates کو override کریں جب کسی مخصوص deployment کو اضافی directives درکار ہوں؛
انہیں موجودہ `--no-*` switches کے ساتھ ملا کر کسی header کو مکمل طور پر ہٹائیں۔

header snippet کو CDN change request کے ساتھ منسلک کریں اور JSON دستاویز کو
gateway automation pipeline میں feed کریں تاکہ اصل host promotion ریلیز evidence
سے میل کھائے۔

xtask helper اب canonical راستہ ہے۔ ریلیز اسکرپٹ verification helper خودکار طور
پر چلاتا ہے تاکہ DG-3 ٹکٹس میں ہمیشہ تازہ evidence شامل ہو۔ جب بھی آپ binding JSON
دستی طور پر تبدیل کریں تو اسے دوبارہ چلائیں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

یہ کمانڈ stapled `Sora-Proof` payload کو ڈی کوڈ کرتی ہے، `Sora-Route-Binding`
metadata کو manifest CID + hostname کے ساتھ میچ کرتی ہے، اور اگر کوئی header drift
ہو تو فوراً fail کرتی ہے۔ جب بھی آپ یہ کمانڈ CI کے باہر چلائیں تو کنسول آؤٹ پٹ کو
دیگر deployment آرٹیفیکٹس کے ساتھ آرکائیو کریں تاکہ DG-3 ریویورز کے پاس ثبوت ہو
کہ binding cutover سے پہلے validate ہوا تھا۔

> **DNS descriptor integration:** `portal.dns-cutover.json` اب `gateway_binding`
> سیکشن شامل کرتا ہے جو ان آرٹیفیکٹس (paths، content CID، proof status، اور
> literal header template) کی طرف اشارہ کرتا ہے **اور** `route_plan` stanza
> `gateway.route_plan.json` اور main + rollback header templates کا حوالہ دیتا ہے۔
> ہر DG-3 change ticket میں یہ بلاکس شامل کریں تاکہ ریویورز عین
> `Sora-Name`/`Sora-Proof`/`CSP` ویلیوز کا فرق دیکھ سکیں اور تصدیق کر سکیں کہ
> route promotion/rollback پلانز evidence bundle کے ساتھ مطابقت رکھتے ہیں بغیر
> build archive کھولے۔

## مرحلہ 9 — publishing monitors چلائیں

روڈ میپ ٹاسک **DOCS-3c** کے لئے مسلسل evidence درکار ہے کہ پورٹل، Try it پروکسی،
اور gateway bindings ریلیز کے بعد صحت مند رہیں۔ مراحل 7-8 کے فوراً بعد consolidated
monitor چلائیں اور اسے scheduled probes میں wire کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` config فائل لوڈ کرتا ہے (schema کے لئے
  `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) اور تین checks
  چلاتا ہے: portal path probes + CSP/Permissions-Policy validation، Try it proxy
  probes (اختیاری طور پر `/metrics` endpoint ہٹ کر)، اور gateway binding verifier
  (`cargo xtask soradns-verify-binding`) جو اب alias/manifest checks کے ساتھ
  Sora-Content-CID کی موجودگی + متوقع ویلیو نافذ کرتا ہے۔
- اگر کوئی بھی probe ناکام ہو تو کمانڈ non-zero سے نکلتی ہے تاکہ CI، cron jobs،
  یا runbook operators aliases پروموٹ کرنے سے پہلے ریلیز روک سکیں۔
- `--json-out` ایک واحد summary JSON payload لکھتا ہے جس میں فی target status ہے؛
  `--evidence-dir` `summary.json`, `portal.json`, `tryit.json`, `binding.json`, اور
  `checksums.sha256` جاری کرتا ہے تاکہ گورننس ریویورز بغیر monitors دوبارہ چلائے
  نتائج diff کر سکیں۔ اس ڈائریکٹری کو `artifacts/sorafs/<tag>/monitoring/` میں
  Sigstore bundle اور DNS cutover descriptor کے ساتھ آرکائیو کریں۔
- monitor آؤٹ پٹ، Grafana export (`dashboards/grafana/docs_portal.json`)، اور
  Alertmanager drill ID کو ریلیز ٹکٹ میں شامل کریں تاکہ DOCS-3c SLO بعد میں
  آڈٹ ہو سکے۔ dedicated publishing monitor playbook یہاں ہے:
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

Portal probes HTTPS کا تقاضا کرتے ہیں اور `http://` بیس URLs کو مسترد کرتے ہیں جب
تک monitor config میں `allowInsecureHttp` سیٹ نہ ہو؛ production/staging targets
کو TLS پر رکھیں اور یہ override صرف لوکل previews کے لئے فعال کریں۔

پورٹل کے لائیو ہونے کے بعد `npm run monitor:publishing` کو Buildkite/cron میں
automate کریں۔ یہی کمانڈ، جب production URLs پر pointed ہو، وہ صحت مند چیکس فراہم
کرتی ہے جن پر SRE/Docs ریلیز کے درمیان انحصار کرتے ہیں۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` مراحل 2-6 کو encapsulate کرتا ہے۔ یہ:

1. `build/` کو deterministic tarball میں آرکائیو کرتا ہے،
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   اور `proof verify` چلاتا ہے،
3. Torii credentials موجود ہوں تو اختیاری طور پر `manifest submit` (alias binding سمیت)
   چلاتا ہے، اور
4. `artifacts/sorafs/portal.pin.report.json`, اختیاری `portal.pin.proposal.json`,
  DNS cutover descriptor (submissions کے بعد)، اور gateway binding bundle
  (`portal.gateway.binding.json` + text header block) لکھتا ہے تاکہ governance,
  networking, اور ops ٹیمیں CI لاگز کھنگالے بغیر evidence bundle کا فرق دیکھ سکیں۔

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, اور (اختیاری)
`PIN_ALIAS_PROOF_PATH` سیٹ کریں۔ dry runs کے لئے `--skip-submit` استعمال کریں؛
GitHub workflow نیچے بیان کردہ `perform_submit` input سے اسے ٹوگل کرتا ہے۔

## مرحلہ 8 — OpenAPI specs اور SBOM bundles شائع کریں

DOCS-7 تقاضہ کرتا ہے کہ پورٹل build، OpenAPI spec، اور SBOM آرٹیفیکٹس ایک ہی
deterministic pipeline سے گزریں۔ موجودہ helpers تینوں کو کور کرتے ہیں:

1. **spec دوبارہ بنائیں اور سائن کریں۔**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   جب بھی تاریخی snapshot محفوظ کرنا ہو تو `--version=<label>` کے ذریعے ریلیز
   لیبل فراہم کریں (مثلاً `2025-q3`)۔ helper snapshot کو
   `static/openapi/versions/<label>/torii.json` میں لکھتا ہے، اسے `versions/current`
   میں mirror کرتا ہے، اور metadata (SHA-256, manifest status, updated timestamp)
   `static/openapi/versions.json` میں ریکارڈ کرتا ہے۔ ڈیولپر پورٹل یہ index پڑھتا
   ہے تاکہ Swagger/RapiDoc پینلز version picker دکھا سکیں اور متعلقہ digest/signature
   info inline پیش کر سکیں۔ `--version` چھوڑ دینے سے پچھلے ریلیز لیبل برقرار رہتے
   ہیں اور صرف `current` + `latest` pointers تازہ ہوتے ہیں۔

   manifest SHA-256/BLAKE3 digests محفوظ کرتا ہے تاکہ gateway `/reference/torii-swagger`
   کے لئے `Sora-Proof` headers stapled کر سکے۔

2. **CycloneDX SBOMs خارج کریں۔** ریلیز pipeline پہلے ہی syft-based SBOMs کی توقع
   رکھتی ہے، جیسا کہ `docs/source/sorafs_release_pipeline_plan.md` میں درج ہے۔
   آؤٹ پٹ کو build artifacts کے ساتھ رکھیں:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **ہر payload کو CAR میں پیک کریں۔**

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
   ہر اثاثے کے لئے aliases ٹیون کریں (مثلاً spec کے لئے `docs-openapi.sora` اور
   سائن شدہ SBOM بنڈل کے لئے `docs-sbom.sora`)۔ الگ aliases رکھنے سے SoraDNS proofs,
   GARs، اور rollback ٹکٹس مخصوص payload تک محدود رہتے ہیں۔

4. **جمع کریں اور bind کریں۔** وہی authority + Sigstore bundle دوبارہ استعمال کریں،
   مگر ریلیز checklist میں alias tuple ریکارڈ کریں تاکہ آڈیٹرز دیکھ سکیں کہ کون سا
   Sora نام کس manifest digest سے منسلک ہے۔

Spec/SBOM manifests کو پورٹل build کے ساتھ آرکائیو کرنا یقینی بناتا ہے کہ ہر
ریلیز ٹکٹ میں مکمل آرٹیفیکٹ سیٹ موجود ہو بغیر packer دوبارہ چلائے۔

### آٹومیشن ہیلپر (CI/package script)

`./ci/package_docs_portal_sorafs.sh` مراحل 1-8 کو encode کرتا ہے تاکہ روڈ میپ
آئٹم **DOCS-7** کو ایک کمانڈ میں چلایا جا سکے۔ ہیلپر:

- مطلوبہ پورٹل prep چلاتا ہے (`npm ci`, OpenAPI/norito sync, widget tests)؛
- پورٹل، OpenAPI، اور SBOM CARs + manifest pairs کو `sorafs_cli` کے ذریعے بناتا ہے؛
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور Sigstore signing
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`) چلاتا ہے؛
- تمام آرٹیفیکٹس کو `artifacts/devportal/sorafs/<timestamp>/` کے تحت چھوڑتا ہے اور
  `package_summary.json` لکھتا ہے تاکہ CI/release tooling بنڈل ingest کر سکے؛ اور
- `artifacts/devportal/sorafs/latest` کو ریفریش کر کے تازہ ترین رن کی طرف اشارہ کرتا ہے۔

مثال (Sigstore + PoR کے ساتھ مکمل pipeline):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

قابلِ توجہ flags:

- `--out <dir>` – artefact root override کریں (default timestamped فولڈرز رکھتا ہے)۔
- `--skip-build` – موجودہ `docs/portal/build` دوبارہ استعمال کریں (جب CI آف لائن
  mirrors کی وجہ سے rebuild نہ کر سکے)۔
- `--skip-sync-openapi` – `npm run sync-openapi` چھوڑ دیں جب `cargo xtask openapi`
  crates.io تک نہ پہنچ سکے۔
- `--skip-sbom` – جب `syft` بائنری انسٹال نہ ہو تو اسے نہ چلائیں (اسکرپٹ warning دیتا ہے)۔
- `--proof` – ہر CAR/manifest جوڑی کے لئے `sorafs_cli proof verify` چلائیں۔ multi-file
  payloads ابھی CLI میں chunk-plan سپورٹ مانگتے ہیں، اس لئے اگر `plan chunk count`
  errors آئیں تو یہ فلیگ بند رکھیں اور upstream gate آنے پر دستی تصدیق کریں۔
- `--sign` – `sorafs_cli manifest sign` چلائیں۔ ٹوکن `SIGSTORE_ID_TOKEN` سے دیں
  (یا `--sigstore-token-env`) یا CLI کو `--sigstore-provider/--sigstore-audience`
  کے ذریعے اسے fetch کرنے دیں۔

production artefacts کے لئے `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
یہ اب پورٹل، OpenAPI، اور SBOM payloads پیک کرتا ہے، ہر manifest پر سائن کرتا ہے،
اور `portal.additional_assets.json` میں اضافی اثاثوں کا metadata ریکارڈ کرتا ہے۔
helper وہی اختیاری knobs سمجھتا ہے جو CI packager استعمال کرتا ہے، ساتھ ہی نئے
`--openapi-*`, `--portal-sbom-*`, اور `--openapi-sbom-*` switches بھی، تاکہ آپ
ہر اثاثے کے لئے alias tuples مقرر کر سکیں، `--openapi-sbom-source` کے ذریعے SBOM
ماخذ override کر سکیں، مخصوص payloads چھوڑ سکیں (`--skip-openapi`/`--skip-sbom`)،
اور non-default `syft` بائنری کی طرف `--syft-bin` سے اشارہ کر سکیں۔

یہ اسکرپٹ ہر کمانڈ دکھاتا ہے جو وہ چلاتا ہے؛ log کو `package_summary.json` کے ساتھ
ریلیز ٹکٹ میں شامل کریں تاکہ ریویورز CAR digests، plan metadata، اور Sigstore bundle
hashes کا فرق ad-hoc shell آؤٹ پٹ دیکھے بغیر نکال سکیں۔

## مرحلہ 9 — Gateway + SoraDNS کی تصدیق

cutover کا اعلان کرنے سے پہلے ثابت کریں کہ نیا alias SoraDNS کے ذریعے resolve ہو
رہا ہے اور gateways تازہ proofs stapled کر رہے ہیں:

1. **probe gate چلائیں۔** `ci/check_sorafs_gateway_probe.sh` ڈیمو fixtures کے خلاف
   `cargo xtask sorafs-gateway-probe` چلاتا ہے جو
   `fixtures/sorafs_gateway/probe_demo/` میں موجود ہیں۔ حقیقی deployments کے لئے
   probe کو ہدف hostname پر چلائیں:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   probe `Sora-Name`, `Sora-Proof`, اور `Sora-Proof-Status` کو
   `docs/source/sorafs_alias_policy.md` کے مطابق decode کرتا ہے اور اگر manifest digest,
   TTLs، یا GAR bindings میں drift ہو تو ناکام ہو جاتا ہے۔

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **drill evidence جمع کریں۔** آپریٹر drills یا PagerDuty dry runs کے لئے probe کو
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- …`
   کے ساتھ wrap کریں۔ wrapper headers/logs کو
   `artifacts/sorafs_gateway_probe/<stamp>/` میں رکھتا ہے، `ops/drill-log.md` اپ ڈیٹ
   کرتا ہے، اور (اختیاری) rollback hooks یا PagerDuty payloads بھی چلاتا ہے۔ SoraDNS
   راستے کی تصدیق کے لئے `--host docs.sora` دیں تاکہ IP hard-code نہ ہو۔

3. **DNS bindings کی تصدیق کریں۔** جب گورننس alias proof شائع کرے تو probe میں
   حوالہ دیا گیا GAR فائل (`--gar`) ریکارڈ کریں اور ریلیز evidence کے ساتھ منسلک کریں۔
   Resolver مالکان اسی ان پٹ کو `tools/soradns-resolver` سے چلا کر تصدیق کر سکتے ہیں
   کہ cached entries نئے manifest کو honour کریں۔ JSON منسلک کرنے سے پہلے چلائیں
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   تاکہ deterministic host mapping، manifest metadata، اور telemetry labels offline
   validate ہو جائیں۔ helper signed GAR کے ساتھ `--json-out` summary بھی نکال سکتا ہے
   تاکہ reviewers بغیر binary کھولے قابلِ تصدیق evidence حاصل کریں۔
  نیا GAR بناتے وقت ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` پر صرف تب جائیں جب manifest فائل دستیاب نہ ہو)۔ helper اب
  CID **اور** BLAKE3 digest دونوں manifest JSON سے براہ راست نکالتا ہے، whitespace
  trim کرتا ہے، دہرے `--telemetry-label` flags ختم کرتا ہے، labels sort کرتا ہے، اور
  JSON لکھنے سے پہلے default CSP/HSTS/Permissions-Policy templates نکالتا ہے تاکہ
  payload deterministic رہے، چاہے آپریٹرز labels مختلف shells سے اکٹھے کریں۔

4. **alias metrics پر نظر رکھیں۔** `torii_sorafs_alias_cache_refresh_duration_ms`
   اور `torii_sorafs_gateway_refusals_total{profile="docs"}` کو probe کے دوران
   اسکرین پر رکھیں؛ دونوں series `dashboards/grafana/docs_portal.json` میں چارٹ ہیں۔

## مرحلہ 10 — Monitoring اور evidence bundling

- **Dashboards۔** ہر ریلیز کے لئے `dashboards/grafana/docs_portal.json` (portal SLOs)،
  `dashboards/grafana/sorafs_gateway_observability.json` (gateway latency + proof health)،
  اور `dashboards/grafana/sorafs_fetch_observability.json` (orchestrator health)
  export کریں۔ JSON exports کو ریلیز ٹکٹ کے ساتھ منسلک کریں تاکہ ریویورز
  Prometheus queries دوبارہ چلا سکیں۔
- **Probe archives۔** `artifacts/sorafs_gateway_probe/<stamp>/` کو git-annex یا
  اپنے evidence bucket میں رکھیں۔ probe summary، headers، اور telemetry script سے
  پکڑا گیا PagerDuty payload شامل کریں۔
- **Release bundle۔** پورٹل/SBOM/OpenAPI CAR summaries، manifest bundles، Sigstore
  signatures، `portal.pin.report.json`, Try-It probe logs، اور link-check reports کو
  ایک timestamped فولڈر (مثال `artifacts/sorafs/devportal/20260212T1103Z/`) میں رکھیں۔
- **Drill log۔** جب probes کسی drill کا حصہ ہوں تو
  `scripts/telemetry/run_sorafs_gateway_probe.sh` کو `ops/drill-log.md` میں append
  کرنے دیں تاکہ وہی evidence SNNet-5 chaos requirement پوری کرے۔
- **Ticket links۔** change ticket میں Grafana panel IDs یا attached PNG exports کا
  حوالہ دیں، ساتھ ہی probe report path بھی شامل کریں، تاکہ reviewers shell access
  کے بغیر SLOs cross-check کر سکیں۔

## مرحلہ 11 — multi-source fetch drill اور scoreboard evidence

SoraFS پر اشاعت اب multi-source fetch evidence (DOCS-7/SF-6) کا تقاضہ کرتی ہے،
اور یہ اوپر والے DNS/gateway proofs کے ساتھ ہونا چاہیے۔ manifest pin کرنے کے بعد:

1. **live manifest کے خلاف `sorafs_fetch` چلائیں۔** مراحل 2-3 کے plan/manifest
   آرٹیفیکٹس اور ہر provider کے جاری کردہ gateway credentials استعمال کریں۔ ہر
   آؤٹ پٹ محفوظ کریں تاکہ آڈیٹرز orchestrator کے decision trail کو replay کر سکیں:

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

   - manifest میں حوالہ دی گئی provider adverts پہلے حاصل کریں (مثلاً
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور انہیں `--provider-advert name=path` کے ذریعے پاس کریں تاکہ scoreboard
     deterministic انداز میں capability windows evaluate کر سکے۔
     `--allow-implicit-provider-metadata` **صرف** CI میں fixtures replay کرتے وقت
     استعمال کریں؛ production drills میں pin کے ساتھ آنے والے signed adverts ہی دیں۔
   - جب manifest اضافی regions کی طرف اشارہ کرے تو متعلقہ provider tuples کے ساتھ
     کمانڈ دہرائیں تاکہ ہر cache/alias کی matching fetch artifact ہو۔

2. **آؤٹ پٹس آرکائیو کریں۔** `scoreboard.json`, `providers.ndjson`, `fetch.json`, اور
   `chunk_receipts.ndjson` کو ریلیز evidence فولڈر کے اندر رکھیں۔ یہ فائلیں peer
   weighting، retry budget، latency EWMA، اور per-chunk receipts محفوظ کرتی ہیں
   جنہیں گورننس پیکٹ SF-7 کے لئے برقرار رکھے۔

3. **ٹیلیمیٹری اپ ڈیٹ کریں۔** fetch outputs کو **SoraFS Fetch Observability**
   ڈیش بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) میں امپورٹ کریں،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` اور provider-range panels پر
   anomalies دیکھیں۔ Grafana panel snapshots کو ریلیز ٹکٹ میں scoreboard path کے ساتھ
   لنک کریں۔

4. **alert rules کا smoke کریں۔** `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   چلا کر ریلیز بند کرنے سے پہلے Prometheus alert bundle validate کریں۔ promtool
   آؤٹ پٹ کو ٹکٹ کے ساتھ منسلک کریں تاکہ DOCS-7 ریویورز تصدیق کر سکیں کہ stall اور
   slow-provider alerts مسلح رہیں۔

5. **CI میں wire کریں۔** پورٹل pin workflow میں `sorafs_fetch` مرحلہ `perform_fetch_probe`
   input کے پیچھے رکھا گیا ہے؛ staging/production رنز میں اسے فعال کریں تاکہ fetch
   evidence manifest bundle کے ساتھ خودکار طور پر تیار ہو۔ لوکل drills وہی اسکرپٹ
   استعمال کر سکتے ہیں جب gateway tokens export ہوں اور `PIN_FETCH_PROVIDERS` کو
   comma-separated provider list سے سیٹ کیا جائے۔

## پروموشن، observability، اور rollback

1. **پروموشن:** staging اور production کے لئے علیحدہ aliases رکھیں۔ پروموشن کے لئے
   اسی manifest/bundle کے ساتھ `manifest submit` دوبارہ چلائیں اور
   `--alias-namespace/--alias-name` کو production alias پر سوئچ کریں۔ اس سے QA منظور
   شدہ staging pin کے بعد دوبارہ build یا re-sign کی ضرورت نہیں رہتی۔
2. **Monitoring:** pin-registry ڈیش بورڈ
   (`docs/source/grafana_sorafs_pin_registry.json`) اور پورٹل مخصوص probes
   (`docs/portal/docs/devportal/observability.md` دیکھیں) امپورٹ کریں۔ checksum drift,
   failed probes، یا proof retry spikes پر alert کریں۔
3. **Rollback:** واپس جانے کے لئے پچھلا manifest دوبارہ submit کریں (یا موجودہ alias
   retire کریں) `sorafs_cli manifest submit --alias ... --retire` کے ذریعے۔ ہمیشہ
   last known-good bundle اور CAR summary محفوظ رکھیں تاکہ CI لاگز rotate ہو جائیں تو
   rollback proofs دوبارہ بن سکیں۔

## CI workflow template

کم از کم، آپ کی pipeline کو یہ کرنا چاہیے:

1. Build + lint (`npm ci`, `npm run build`, checksum generation).
2. Package (`car pack`) اور manifests compute کریں۔
3. Job-scoped OIDC ٹوکن کے ساتھ sign کریں (`manifest sign`).
4. Auditing کے لئے artefacts اپ لوڈ کریں (CAR, manifest, bundle, plan, summaries).
5. pin registry میں submit کریں:
   - Pull requests → `docs-preview.sora`.
   - Tags / protected branches → production alias promotion.
6. exit سے پہلے probes + proof verification gates چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` manual releases کے لئے یہ تمام
مراحل جوڑتا ہے۔ workflow:

- پورٹل build/test کرتا ہے،
- `scripts/sorafs-pin-release.sh` کے ذریعے build پیکج کرتا ہے،
- GitHub OIDC استعمال کر کے manifest bundle سائن/verify کرتا ہے،
- CAR/manifest/bundle/plan/proof summaries کو artifacts کے طور پر اپ لوڈ کرتا ہے، اور
- (اختیاری) secrets موجود ہوں تو manifest + alias binding submit کرتا ہے۔

job چلانے سے پہلے درج ذیل repository secrets/variables کنفیگر کریں:

| Name | Purpose |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii host جو `/v1/sorafs/pin/register` ظاہر کرتا ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | submissions کے ساتھ ریکارڈ ہونے والا epoch identifier۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | manifest submission کے لئے signing authority۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tuple جو `perform_submit` true ہونے پر manifest سے bind ہوتا ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-encoded alias proof bundle (اختیاری؛ alias binding چھوڑنے کے لئے omit کریں)۔ |
| `DOCS_ANALYTICS_*` | موجودہ analytics/probe endpoints جو دوسرے workflows میں reuse ہوتے ہیں۔ |

Actions UI سے workflow trigger کریں:

1. `alias_label` فراہم کریں (مثلاً `docs.sora.link`)، optional `proposal_alias`,
   اور optional `release_tag` override۔
2. artefacts بنانے کے لئے `perform_submit` کو unchecked چھوڑیں (Torii کو touch کیے بغیر)
   یا اسے enable کریں تاکہ configured alias پر براہ راست publish ہو جائے۔

`docs/source/sorafs_ci_templates.md` اب بھی اس repo کے باہر کے پروجیکٹس کے لئے
generic CI helpers دستاویز کرتا ہے، مگر روزمرہ ریلیز کے لئے پورٹل workflow ترجیحی ہے۔

## Checklist

- [ ] `npm run build`, `npm run test:*`, اور `npm run check:links` گرین ہیں۔
- [ ] `build/checksums.sha256` اور `build/release.json` artefacts میں محفوظ ہیں۔
- [ ] CAR, plan, manifest, اور summary `artifacts/` کے تحت بنے ہیں۔
- [ ] Sigstore bundle + detached signature لاگز کے ساتھ محفوظ ہیں۔
- [ ] `portal.manifest.submit.summary.json` اور `portal.manifest.submit.response.json`
      submissions کے وقت محفوظ ہوئے۔
- [ ] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
      CAR/manifest artefacts کے ساتھ آرکائیو ہوئے۔
- [ ] `proof verify` اور `manifest verify-signature` لاگز آرکائیو ہوئے۔
- [ ] Grafana dashboards اپ ڈیٹ ہیں + Try-It probes کامیاب ہیں۔
- [ ] rollback notes (previous manifest ID + alias digest) ریلیز ٹکٹ کے ساتھ منسلک ہیں۔
