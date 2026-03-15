---
id: deploy-guide
lang: kk
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Шолу

Бұл кітап жол картасының элементтерін **DOCS-7** (SoraFS жариялау) және **DOCS-8** түрлендіреді.
(CI/CD пин автоматизациясы) әзірлеуші порталының әрекет етуші процедурасына айналдырыңыз.
Ол құрастыру/линт фазасын, SoraFS қаптамасын, Sigstore қолдау көрсететін манифестті қамтиды.
қол қою, бүркеншік атпен жылжыту, тексеру және кері қайтару жаттығулары осылайша әрбір алдын ала қарау және
шығару артефакті қайталанатын және тексерілетін.

Ағын сізде `sorafs_cli` екілік нұсқасы бар (құрылған
`--features cli`), PIN-тізілім рұқсаттары бар Torii соңғы нүктесіне қатынасу және
OIDC Sigstore үшін тіркелгі деректері. Ұзақ өмір сүретін құпияларды сақтаңыз (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii белгілері) сіздің CI қоймаңызда; жергілікті жүгірулер олардың көзі бола алады
қабық экспортынан.

## Алғышарттар

- `npm` немесе `pnpm` бар 18.18+ түйіні.
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli` бастап.
- Torii URL мекенжайы `/v2/sorafs/*` плюс өкілетті тіркелгіні/жеке кілтті көрсетеді
  манифесттер мен бүркеншік аттарды жібере алады.
- OIDC эмитенті (GitHub әрекеттері, GitLab, жұмыс жүктемесінің идентификациясы және т.б.)
  `SIGSTORE_ID_TOKEN`.
- Қосымша: құрғақ жүгіруге арналған `examples/sorafs_cli_quickstart.sh` және
  `docs/source/sorafs_ci_templates.md` GitHub/GitLab жұмыс процесінің тірегі үшін.
- Try it OAuth айнымалы мәндерін конфигурациялаңыз (`DOCS_OAUTH_*`) және іске қосыңыз
  [қауіпсіздікті күшейтетін бақылау тізімі](./security-hardening.md) құрастыруды жылжытудан бұрын
  зертханадан тыс. Бұл айнымалылар жоқ кезде портал құрастыру енді сәтсіз болады
  немесе TTL/сауалдау тұтқалары бекітілген терезелердің сыртына түскенде; экспорт
  `DOCS_OAUTH_ALLOW_INSECURE=1` тек бір рет қолданылатын жергілікті алдын ала қарау үшін. тіркеңіз
  босату билетіне қалам-тест дәлелі.

## 0-қадам — Байқап көріңіз прокси бумасын түсіріңіз

Алдын ала қарауды Netlify немесе шлюзге жылжытпас бұрын, көріңіз проксиге мөр қойыңыз
көздер мен қол қойылған OpenAPI манифест дайджесті детерминирленген бумаға:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` прокси/зонд/қайтару көмекшілерін көшіреді,
OpenAPI қолтаңбасын тексереді және `release.json` плюс жазады
`checksums.sha256`. Бұл топтаманы Netlify/SoraFS шлюз жарнамасына тіркеңіз
тексерушілер дәл прокси көздерін және Torii мақсатты кеңестерін қайталай алатындай билет
қайта құрусыз. Бума сонымен қатар клиент жеткізген жеткізушілер болған-болмайтынын жазады
қосу жоспары мен CSP ережелерін синхрондауда сақтау үшін (`allow_client_auth`).

## 1-қадам — Порталды құрастырыңыз және сызыңыз

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

`npm run build` `scripts/write-checksums.mjs` функциясын автоматты түрде орындайды, мыналарды жасайды:

- `build/checksums.sha256` — `sha256sum -c` үшін жарамды SHA256 манифесті.
- `build/release.json` — метадеректер (`tag`, `generated_at`, `source`)
  әрбір CAR/манифест.

Тексерушілер алдын ала қарауды басқаша алуы үшін екі файлды да CAR қорытындысымен бірге мұрағаттаңыз
қайта құрусыз артефактілер.

## 2-қадам — Статикалық активтерді жинаңыз

CAR пакетін Docusaurus шығыс каталогына қарсы іске қосыңыз. Төмендегі мысал
`artifacts/devportal/` астында барлық артефактілерді жазады.

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

Жиынтық JSON кесінділерді, дайджесттерді және дәлелдеуді жоспарлау бойынша кеңестерді қамтиды
`manifest build` және CI бақылау тақталары кейінірек қайта пайдаланылады.

## 2b-қадам — OpenAPI пакеті және SBOM серіктестері

DOCS-7 портал сайтын, OpenAPI суретін және SBOM пайдалы жүктемелерін жариялауды талап етеді
Шлюздер `Sora-Proof`/`Sora-Content-CID` степлей алады.
әрбір артефакт үшін тақырыптар. Шығарылатын көмекші
(`scripts/sorafs-pin-release.sh`) OpenAPI каталогын буып қойған.
(`static/openapi/`) және `syft` арқылы шығарылатын SBOMs бөлек
`openapi.*`/`*-sbom.*` CAR және метадеректерді мына қалтада жазады
`artifacts/sorafs/portal.additional_assets.json`. Қолмен ағынды іске қосқанда,
өз префикстері мен метадеректер белгілері бар әрбір пайдалы жүктеме үшін 2-4 қадамдарды қайталаңыз
(мысалы, `--car-out "$OUT"/openapi.car` плюс
`--metadata alias_label=docs.sora.link/openapi`). Әрбір манифестті/бүркеншік атты тіркеңіз
DNS ауыстырмас бұрын Torii (сайт, OpenAPI, SBOM порталы, OpenAPI SBOM) ішінде жұптаңыз.
шлюз барлық жарияланған артефактілер үшін қапсырмаланған дәлелдерге қызмет ете алады.

## 3-қадам — Манифест құрастыру

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

Шығарылым терезесіне pin-политика жалаушаларын реттеңіз (мысалы, `--pin-storage-class
канарейлерге арналған ыстық`). JSON нұсқасы қосымша, бірақ кодты қарап шығуға ыңғайлы.

## 4-қадам — Sigstore арқылы қол қойыңыз

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Бума манифест дайджестін, кесек дайджесттерді және BLAKE3 хэшін жазады.
JWT сақталмай OIDC таңбалауышы. Буманы да, бөлек те сақтаңыз
қол қою; өндірісті жылжыту жұмыстан шығудың орнына сол артефактілерді қайта пайдалана алады.
Жергілікті іске қосулар провайдер жалаушаларын `--identity-token-env` (немесе орнату) ауыстыра алады
`SIGSTORE_ID_TOKEN` ортада) сыртқы OIDC көмекшісі
жетон.

## 5-қадам — PIN тізіліміне жіберіңіз

Қол қойылған манифестті (және бөліктік жоспарды) Torii мекенжайына жіберіңіз. Әрқашан қорытынды сұраңыз
сондықтан алынған тізілім жазбасы/бүркеншік аттың дәлелі тексеріледі.

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

Алдын ала қарауды немесе канарей бүркеншік атын (`docs-preview.sora`) шығарған кезде
бірегей бүркеншік атпен жіберу, осылайша QA өнім шығару алдында мазмұнды тексере алады
жылжыту.

Бүркеншік атпен байланыстыру үш өрісті қажет етеді: `--alias-namespace`, `--alias-name` және
`--alias-proof`. Басқару дәлелдеу бумасын шығарады (base64 немесе Norito байт)
бүркеншік ат сұрау мақұлданған кезде; оны CI құпияларында сақтаңыз және оны а ретінде көрсетіңіз
`manifest submit` шақыру алдында файл. Кез келген кезде бүркеншік ат жалаушаларын орнатусыз қалдырыңыз
тек DNS-ге қол тигізбестен манифестті бекітуді көздейді.

## Қадам 5b — Басқару ұсынысын жасаңыз

Әрбір манифест кез келген Сора болатындай Парламент дайын ұсыныспен саяхаттау керек
азамат артықшылықты куәлікті қарызға алмай-ақ өзгеріс енгізе алады.
Жіберу/қол қою қадамдарынан кейін іске қосыңыз:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` канондық `RegisterPinManifest` түсіреді
нұсқаулық, кесінді дайджест, саясат және бүркеншік ат туралы кеңес. Оны басқармаға бекітіңіз
билет немесе Парламент порталы, осылайша делегаттар пайдалы жүктемені қайта құрусыз ажырата алады
артефактілер. Өйткені пәрмен Torii өкілетті кілтіне ешқашан тимейді, кез келген
азамат жергілікті жерде ұсыныс жасай алады.

## 6-қадам — Дәлелдемелер мен телеметрияны тексеріңіз

Бекіткеннен кейін детерминирленген тексеру қадамдарын орындаңыз:

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

- `torii_sorafs_gateway_refusals_total` және тексеріңіз
  Аномалиялар үшін `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Try-It проксиін және жазылған сілтемелерді пайдалану үшін `npm run probe:portal` іске қосыңыз
  жаңадан бекітілген мазмұнға қарсы.
- бөлімінде сипатталған бақылау дәлелдерін түсіріңіз
  [Жариялау және бақылау](./publishing-monitoring.md) сондықтан DOCS-3c
  Бақылау қақпасы жариялау қадамдарымен қатар қанағаттандырылады. Көмекші
  енді бірнеше `bindings` жазбаларын қабылдайды (сайт, OpenAPI, SBOM порталы, OpenAPI
  SBOM) және мақсатқа `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` күшіне енгізеді
  қосымша `hostname` қорғаушысы арқылы хост. Төмендегі шақыру а деп те жазады
  жалғыз JSON қорытындысы және дәлелдер жинағы (`portal.json`, `tryit.json`,
  `binding.json` және `checksums.sha256`) шығарылым каталогында:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Қадам 6a — Шлюз сертификаттарын жоспарлаңыз

TLS SAN/challenge жоспарын шлюз ретінде GAR пакеттерін жасамас бұрын шығарыңыз
команда және DNS мақұлдаушылары бірдей дәлелдерді қарайды. Жаңа көмекші бейнені көрсетеді
Канондық қойылмалы таңбалы хосттарды санау арқылы DG-3 автоматтандыру кірістері,
әдемі хост SAN, DNS-01 белгілері және ұсынылған ACME тапсырмалары:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON файлын шығарылым жинағымен бірге орындаңыз (немесе оны өзгертумен бірге жүктеп салыңыз
билет) операторлар SAN мәндерін Torii мәндеріне қоя алады
`torii.sorafs_gateway.acme` конфигурациясы және GAR шолушылары растай алады
хост туындыларын қайта іске қоспай, канондық/әдемі салыстырулар. Қосымша қосыңыз
Бір шығарылымда көтерілген әрбір жұрнақ үшін `--name` дәлелдері.

## Қадам 6b — Канондық хост салыстыруларын шығару

GAR пайдалы жүктемелерін үлгілеуден бұрын, әрқайсысы үшін детерминирленген хост салыстыруын жазыңыз
бүркеншік ат. `cargo xtask soradns-hosts` әр `--name` хэштерін өзінің канондық форматына
жапсырма (`<base32>.gw.sora.id`), қажетті қойылмалы таңбаны шығарады
(`*.gw.sora.id`) және әдемі хостты шығарады (`<alias>.gw.sora.name`). Табандылық
шығарылым артефактілеріндегі шығыс DG-3 рецензенттері салыстыруды ажырата алады
GAR ұсынуымен қатар:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

GAR немесе шлюз кезінде жылдам істен шығу үшін `--verify-host-patterns <file>` пайдаланыңыз
байланыстыру JSON қажетті хосттардың бірін өткізбейді. Көмекші бірнеше қабылдайды
тексеру файлдары, бұл GAR үлгісін де, үлгіні де сызуды жеңілдетеді
сол шақырудағы `portal.gateway.binding.json` қапсырмасы:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Жиынтық JSON және тексеру журналын DNS/шлюз өзгерту билетіне тіркеңіз
аудиторлар канондық, қойылмалы және әдемі хосттарды қайта іске қоспай-ақ растай алады
туынды сценарийлер. Жаңа бүркеншік аттар қосылған сайын пәрменді қайта іске қосыңыз
келесі GAR жаңартулары бірдей дәлелдер ізін иеленеді.

## 7-қадам — DNS кесу дескрипторын жасаңыз

Өндірістік үзілістер тексерілетін өзгерту пакетін қажет етеді. Сәтті болғаннан кейін
тапсыру (бүркеншік ат байланыстыру), көмекші шығарады
`artifacts/sorafs/portal.dns-cutover.json`, түсіру:- бүркеншік атпен байланыстыратын метадеректер (аттар кеңістігі/аты/дәлелдеу, манифест дайджест, Torii URL,
  берілген дәуір, билік);
- шығарылым мәтінмәні (тег, бүркеншік ат белгісі, манифест/CAR жолдары, блок жоспары, Sigstore
  бума);
- тексеру көрсеткіштері (зерттеу командасы, бүркеншік ат + Torii соңғы нүкте); және
- қосымша өзгертуді басқару өрістері (билет идентификаторы, кесу терезесі, операциялық байланыс,
  өндірістік хост атауы/аймақ);
- `Sora-Route-Binding` қапсырмасынан алынған маршрутты жылжыту метадеректері
  тақырып (канондық хост/CID, тақырып + байланыстыру жолдары, тексеру пәрмендері),
  GAR ілгерілету және қалпына келтіру жаттығуларының бірдей дәлелдерге сілтеме жасауын қамтамасыз ету;
- құрылған маршрут жоспарының артефактілері (`gateway.route_plan.json`,
  тақырып үлгілері және қосымша кері тақырыптар) сондықтан билеттерді және CI мәнін өзгертіңіз
  линт ілмектері әрбір DG-3 пакетінің канондық пакетке сілтеме жасайтынын тексере алады
  бекітуге дейін жылжыту/қайтару жоспарлары;
- қосымша кэшті жарамсыз ету метадеректері (тазалаудың соңғы нүктесі, аутентификация айнымалысы, JSON
  пайдалы жүктеме және мысал `curl` пәрмені); және
- алдыңғы дескрипторды көрсететін кері қайтару кеңестері (шығару тегін және манифест
  дайджест) сондықтан өзгерту билеттері детерминирленген қалпына келтіру жолын алады.

Шығарылым кэшті тазалауды қажет еткенде, келесімен бірге канондық жоспар жасаңыз
кесінді дескрипторы:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Алынған `portal.cache_plan.json` DG-3 пакетіне операторлар үшін тіркеңіз
шығару кезінде детерминирленген хосттар/жолдар (және сәйкес аутентификациялық кеңестер) бар
`PURGE` сұраулары. Дескриптордың қосымша кэш метадеректер бөлімі сілтеме жасай алады
бұл файлды тікелей өзгерте отырып, өзгертулерді бақылауды тексерушілерді дәл қайсысына сәйкестендіреді
кесу кезінде соңғы нүктелер тазаланады.

Әрбір DG-3 пакетіне жылжыту + кері қайтару тексеру парағы қажет. арқылы жасаңыз
`cargo xtask soradns-route-plan`, сондықтан өзгертуді бақылау шолушылары дәл бақылай алады
бүркеншік ат бойынша алдын ала ұшу, кесу және кері қайтару қадамдары:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Шығарылған `gateway.route_plan.json` сахналанған канондық/әдемі хосттарды түсіреді
денсаулықты тексеру еске салғыштары, GAR байланыстыру жаңартулары, кэшті тазарту және кері қайтару әрекеттері.
Өзгерістерді жібермес бұрын оны GAR/байлау/қиып алу артефактілерімен біріктіріңіз
билет, осылайша Ops сценариймен жазылған қадамдарды орындап, шығуға мүмкіндік береді.

`scripts/generate-dns-cutover-plan.mjs` осы дескрипторды қуаттайды және жұмыс істейді
`sorafs-pin-release.sh` бастап автоматты түрде. Оны қалпына келтіру немесе теңшеу үшін
қолмен:

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

PIN кодын іске қоспас бұрын орта айнымалылары арқылы қосымша метадеректерді толтырыңыз
көмекші:

| Айнымалы | Мақсаты |
|----------|---------|
| `DNS_CHANGE_TICKET` | Дескрипторда сақталған билет идентификаторы. |
| `DNS_CUTOVER_WINDOW` | ISO8601 кесу терезесі (мысалы, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Өндірістік хост атауы + беделді аймақ. |
| `DNS_OPS_CONTACT` | Қоңыраудағы бүркеншік ат немесе эскалация контактісі. |
| `DNS_CACHE_PURGE_ENDPOINT` | Дескрипторда жазылған кэшті тазартудың соңғы нүктесі. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Тазарту таңбалауышын қамтитын Env var (әдепкі `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Қайтару метадеректеріне арналған алдыңғы кесу дескрипторына жол. |

Бекітушілер манифестті тексере алуы үшін JSON файлын DNS өзгерту шолуына тіркеңіз
дайджесттер, бүркеншік атпен байланыстырулар және CI журналдарын сызусыз тексеру пәрмендері.
CLI жалаулары `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` және `--previous-dns-plan` бірдей қайта анықтауды қамтамасыз етеді
көмекшіні CI сыртында іске қосқанда.

## 8-қадам — шешуші аймақтық файл қаңқасын шығару (міндетті емес)

Өндірістің қысқару терезесі белгілі болған кезде, шығарылым сценарийі шығара алады
SNS аймақтық файлының қаңқасы және шешуші үзіндісі автоматты түрде. Қажетті DNS жіберіңіз
орта айнымалылары немесе CLI опциялары арқылы жазбалар мен метадеректер; көмекші
кесілгеннен кейін бірден `scripts/sns_zonefile_skeleton.py` нөміріне қоңырау шалады
дескриптор жасалады. Кем дегенде бір A/AAAA/CNAME мәнін және GAR мәнін беріңіз
дайджест (қол қойылған GAR пайдалы жүктемесінің BLAKE3-256). Егер аймақ/хост аты белгілі болса
және `--dns-zonefile-out` алынып тасталды, көмекші оған жазады
`artifacts/sns/zonefiles/<zone>/<hostname>.json` және толтырады
`ops/soradns/static_zones.<hostname>.json` шешуші үзінді ретінде.

| Айнымалы / жалауша | Мақсаты |
|----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Жасалған аймақтық файл қаңқасының жолы. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Шешуші үзінді жолы (өткізілген кезде әдепкі `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL жасалған жазбаларға қолданылады (әдепкі: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 мекенжайлары (үтірмен бөлінген env немесе қайталанатын CLI жалауы). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 мекенжайлары. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Қосымша CNAME мақсаты. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI түйреуіштері (негізгі 64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Қосымша TXT жазбалары (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Есептелген аймақтық файл нұсқасының белгісін қайта анықтау. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Кесетін терезені бастаудың орнына `effective_at` уақыт белгісін (RFC3339) мәжбүрлеңіз. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Метадеректерде жазылған дәлелдеу литералын қайта анықтау. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Метадеректерде жазылған CID қайта анықтау. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Күзетші мұздату күйі (жұмсақ, қатты, еріту, бақылау, төтенше жағдай). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Қауіпсіздікке арналған қамқоршы/кеңес билетінің анықтамасы. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Ерітуге арналған RFC3339 уақыт белгісі. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Қосымша мұздату жазбалары (үтірмен бөлінген env немесе қайталанатын жалауша). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Қол қойылған GAR пайдалы жүктемесінің BLAKE3-256 дайджесті (он алтылық). Шлюз байлаулары болған кезде қажет. |

GitHub Actions жұмыс процесі бұл мәндерді репозитарий құпияларынан оқиды, осылайша әрбір өндірістік түйреуіш аймақтық файл артефактілерін автоматты түрде шығарады. Келесі құпияларды конфигурациялаңыз (жолдарда көп мәнді өрістер үшін үтірмен бөлінген тізімдер болуы мүмкін):

| Құпия | Мақсаты |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Өндіріс хост атауы/аймағы көмекшіге берілді. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Дескрипторда сақталған қоңырау бойынша бүркеншік ат. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Жариялауға арналған IPv4/IPv6 жазбалары. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Қосымша CNAME мақсаты. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI түйреуіштері. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Қосымша TXT жазбалары. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Қаңқада жазылған метадеректерді мұздату. |
| `DOCS_SORAFS_GAR_DIGEST` | Қол қойылған GAR пайдалы жүктемесінің он алтылық кодталған BLAKE3 дайджесті. |

`.github/workflows/docs-portal-sorafs-pin.yml` іске қосқан кезде дескриптор/аймақтық файл дұрыс өзгерту терезесінің метадеректерін иеленуі үшін `dns_change_ticket` және `dns_cutover_window` кірістерін қамтамасыз етіңіз. Оларды тек құрғақ жүгірістерді орындаған кезде бос қалдырыңыз.

Әдеттегі шақыру (SN-7 иесінің жұмыс кітабына сәйкес):

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
  …other flags…
```

Көмекші өзгерту билетін TXT жазбасы ретінде автоматты түрде өткізеді және
кесу терезесінің басталуын `effective_at` уақыт белгісі ретінде иеленеді.
ауыстырылды. Толық операциялық жұмыс процесі үшін қараңыз
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Жалпы DNS делегациясының жазбасы

Аймақтық файлдың қаңқасы тек аймақ үшін беделді жазбаларды анықтайды. Сіз
әлі де тіркеушіде немесе DNS-те ата-аналық аймақ NS/DS өкілдігін конфигурациялау қажет
провайдер, сондықтан кәдімгі интернет аттар серверлерін таба алады.

- Апекс/TLD кесінділері үшін ALIAS/ANAME (провайдерге арнайы) пайдаланыңыз немесе A/AAAA жариялаңыз
  кез келген IP шлюзін көрсететін жазбалар.
- Ішкі домендер үшін алынған әдемі хостқа CNAME жариялаңыз
  (`<fqdn>.gw.sora.name`).
- Канондық хост (`<hash>.gw.sora.id`) шлюз доменінің астында қалады және
  жалпыға ортақ аймақта жарияланбаған.

### Шлюз тақырыбы үлгісі

Қолдану көмекшісі де `portal.gateway.headers.txt` және шығарады
`portal.gateway.binding.json`, DG-3 талаптарын қанағаттандыратын екі артефакт
шлюз мазмұнын байланыстыру талабы:

- `portal.gateway.headers.txt` толық HTTP тақырып блогын қамтиды (соның ішінде
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS және
  `Sora-Route-Binding` дескрипторы) шеткі шлюздер әрбір шлюзге қапсырмалануы керек.
  жауап.
- `portal.gateway.binding.json` бірдей ақпаратты машина оқи алатындай етіп жазады
  пішінді өзгертіңіз, сондықтан билеттерді өзгерту және автоматтандыру хост/сид байланыстарын онсыз да ажырата алады
  қабықшаның шығуы.

Олар арқылы автоматты түрде жасалады
`cargo xtask soradns-binding-template`
және берілген бүркеншік атты, манифест дайджестін және шлюз хост атын түсіріңіз
`sorafs-pin-release.sh` дейін. Тақырып блогын қалпына келтіру немесе теңшеу үшін келесі әрекеттерді орындаңыз:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Қайта анықтау үшін `--csp-template`, `--permissions-template` немесе `--hsts-template` өтіңіз
арнайы орналастыру қосымша қажет болғанда әдепкі тақырып үлгілері
директивалар; тақырыпты тастау үшін оларды бар `--no-*` қосқыштарымен біріктіріңіз
толығымен.

Тақырып үзіндісін CDN өзгерту сұрауына тіркеңіз және JSON құжатын беріңіз
шлюзді автоматтандыру құбырына енгізіңіз, осылайша нақты хост жылжыту сәйкес келеді
дәлелдемелерді босату.

Шығарылым сценарийі DG-3 билеттері үшін растау көмекшісін автоматты түрде іске қосады
әрқашан соңғы дәлелдерді қамтиды. параметрін өзгерткен сайын оны қолмен қайта іске қосыңыз
JSON қолмен байланыстыру:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Пәрмен байланыстыру бумасында түсірілген `Sora-Proof` пайдалы жүктемесін тексереді,
`Sora-Route-Binding` метадеректерінің манифест CID + хост атауына сәйкес келуін қамтамасыз етеді,
және кез келген тақырып дрейф болса, тез істен шығады. жанындағы консоль шығысын мұрағаттаңыз
басқа орналастыру артефактілері CI сыртындағы пәрменді іске қосқан сайын DG-3
шолушылардың түптеу кесілгенге дейін расталғанының дәлелі бар.> **DNS дескрипторын біріктіру:** `portal.dns-cutover.json` енді
> Осы артефактілерді көрсететін `gateway_binding` бөлімі (жолдар, мазмұн CID,
> дәлелдеу күйі және әріптік тақырып үлгісі) **және** `route_plan` шумақ
> `gateway.route_plan.json` плюс негізгі + кері қайтару тақырыбына сілтеме жасау
> шаблондар. Бұл блоктарды әрбір DG-3 өзгерту билетіне енгізіңіз, сонда шолушылар мүмкін
> дәл `Sora-Name/Sora-Proof/CSP` мәндерін ажыратыңыз және маршрутты растаңыз
> жылжыту/қайтару жоспарлары құрастыруды ашпай-ақ дәлелдер жинағына сәйкес келеді
> мұрағат.

## 9-қадам — Жариялау мониторларын іске қосыңыз

Жол картасы тапсырмасы **DOCS-3c** порталдың үздіксіз дәлелдеуді талап етеді, Байқап көріңіз
прокси және шлюз байланыстары шығарылғаннан кейін сау болып қалады. Біріктірілгенді іске қосыңыз
7–8-қадамдардан кейін дереу бақылаңыз және оны жоспарланған зондтарға қосыңыз:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` конфигурация файлын жүктейді (қараңыз
  Схема үшін `docs/portal/docs/devportal/publishing-monitoring.md`) және
  үш тексеруді орындайды: портал жолының тексерулері + CSP/Рұқсаттар-Саясатты тексеру,
  Прокси зондтарын қолданып көріңіз (міндетті түрде `/metrics` соңғы нүктесін басыңыз) және
  тексеретін шлюзді байланыстыратын тексеруші (`cargo xtask soradns-verify-binding`).
  күтілетін бүркеншік атқа, хостқа, дәлелдеу күйіне қарсы түсірілген байланыстыру бумасы,
  және манифест JSON.
- Кез келген зонд сәтсіз болған кезде пәрмен нөлден тыс шығады, сондықтан CI, cron тапсырмалары немесе
  runbook операторлары бүркеншік аттарды көтермес бұрын шығарылымды тоқтата алады.
- `--json-out` өту арқылы бір мақсатқа арналған JSON пайдалы жүктемесін жазады
  статус; `--evidence-dir` шығарады `summary.json`, `portal.json`, `tryit.json`,
  `binding.json` және `checksums.sha256`, сондықтан басқаруды сарапшылар ажырата алады.
  мониторларды қайта іске қоспай-ақ нәтиже береді. Бұл каталогты астына мұрағаттаңыз
  `artifacts/sorafs/<tag>/monitoring/` Sigstore пакетімен және DNS
  кесінді дескриптор.
- Монитор шығысын қосыңыз, Grafana экспорты (`dashboards/grafana/docs_portal.json`),
  және DOCS-3c SLO болуы мүмкін шығарылым билетіндегі Alertmanager бұрғылау идентификаторы
  кейінірек тексерілді. Арнайы баспа мониторының ойын кітабы мына жерде тұрады
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Портал зондтары HTTPS талап етеді және `http://` негізгі URL мекенжайларын қабылдамайды
`allowInsecureHttp` монитор конфигурациясында орнатылған; өндірісті/қоюды сақтау
TLS-дегі мақсаттарды белгілейді және тек жергілікті алдын ала қарау үшін қайта анықтауды қосыңыз.

Мониторды Buildkite/cron ішіндегі `npm run monitor:publishing` арқылы автоматтандырыңыз.
портал тікелей эфирде. Өндіріс URL мекенжайларына бағытталған бірдей пәрмен ағымдағы ақпаратты береді
SRE/Docs шығарылымдар арасында сүйенетін денсаулық тексерулері.

## `sorafs-pin-release.sh` көмегімен автоматтандыру

`docs/portal/scripts/sorafs-pin-release.sh` 2–6-қадамдарды инкапсуляциялайды. Ол:

1. `build/` мұрағаты детерминирленген тарболға,
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   және `proof verify`,
3. Torii кезінде міндетті түрде `manifest submit` (бүркеншік атпен байланыстыруды қоса) орындайды
   тіркелгі деректері бар және
4. міндетті емес `artifacts/sorafs/portal.pin.report.json` жазады
  `portal.pin.proposal.json`, DNS кесу дескрипторы (жіберілгеннен кейін),
  және шлюз байланыстыру жинағы (`portal.gateway.binding.json` плюс
  мәтін тақырыбы блогы) сондықтан басқару, желі және операциялық топтар бір-бірінен ерекшеленуі мүмкін
  CI журналдарын сызып алмаған дәлелдер жинағы.

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` және (міндетті емес) орнату
`PIN_ALIAS_PROOF_PATH` сценарийді шақырмас бұрын. Кептіру үшін `--skip-submit` пайдаланыңыз
жүгіру; Төменде сипатталған GitHub жұмыс процесі оны `perform_submit` арқылы ауыстырады
енгізу.

## 8-қадам — OpenAPI сипаттамалары мен SBOM жинақтарын жариялау

DOCS-7 саяхаттау үшін портал құрастыруын, OpenAPI спецификациясын және SBOM артефактілерін қажет етеді
бірдей детерминирленген құбыр арқылы. Қолданыстағы көмекшілер үшеуін де қамтиды:

1. **Спецификацияны қайта жасаңыз және қол қойыңыз.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Сақтағыңыз келген кезде `--version=<label>` арқылы шығару жапсырмасын жіберіңіз a
   тарихи сурет (мысалы, `2025-q3`). Көмекші суретті жазады
   `static/openapi/versions/<label>/torii.json`, оны айналады
   `versions/current` және метадеректерді жазады (SHA-256, манифест күйі және
   жаңартылған уақыт белгісі) `static/openapi/versions.json`. Әзірлеуші порталы
   бұл индексті оқиды, сондықтан Swagger/RapiDoc панельдері нұсқа таңдау құралын ұсына алады
   және қатысты дайджест/қолтаңба ақпаратын кірістірілген түрде көрсетіңіз. Өткізілу
   `--version` алдыңғы шығарылым белгілерін сақтайды және тек жаңартады
   `current` + `latest` көрсеткіштері.

   Манифест SHA-256/BLAKE3 дайджесттерін түсіреді, осылайша шлюз степлей алады
   `Sora-Proof` тақырыптары `/reference/torii-swagger` үшін.

2. **CycloneDX SBOMs шығарыңыз.** Шығару құбыры қазірдің өзінде syft негізіндегі күтуде.
   `docs/source/sorafs_release_pipeline_plan.md` үшін SBOMs. Шығаруды сақтаңыз
   құрылыс артефактілерінің жанында:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Әр пайдалы жүкті көлікке салыңыз.**

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

   Негізгі сайт сияқты `manifest build` / `manifest sign` қадамдарын орындаңыз,
   актив үшін бүркеншік аттарды баптау (мысалы, спецификация үшін `docs-openapi.sora` және
   Қол қойылған SBOM бумасы үшін `docs-sbom.sora`). Ерекше бүркеншік аттарды сақтау
   SoraDNS дәлелдерін, GAR және қайтару билеттерін нақты пайдалы жүктемеге дейін сақтайды.

4. **Жіберу және байланыстыру.** Қолданыстағы рұқсатты + Sigstore бумасын қайта пайдаланыңыз, бірақ
   аудиторлар қайсысын бақылай алатындай етіп шығарылымды тексеру тізіміне бүркеншік ат кортежін жазыңыз
   Манифест дайджесті Сора атауы карталары.

Портал құрастыруымен бірге спецификация/SBOM манифесін мұрағаттау барлығын қамтамасыз етеді
шығару билетінде бумалауышты қайта іске қоспай-ақ толық артефакт жинағы бар.

### Автоматтандыру көмекшісі (CI/пакет сценарийі)

`./ci/package_docs_portal_sorafs.sh` 1-8 қадамдарды кодтайды, сондықтан жол картасы элементі
**DOCS‑7** бір пәрменмен орындалуы мүмкін. Көмекші:

- қажетті порталды дайындауды іске қосады (`npm ci`, OpenAPI/norito синхрондау, виджет сынақтары);
- `sorafs_cli` арқылы порталды, OpenAPI және SBOM CARs + манифест жұптарын шығарады;
- міндетті түрде `sorafs_cli proof verify` (`--proof`) және Sigstore қол қоюды іске қосады
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- `artifacts/devportal/sorafs/<timestamp>/` және астында әрбір артефакт түсіреді
  `package_summary.json` деп жазады, сондықтан CI/шығару құралдары жинақты қабылдай алады; және
- ең соңғы іске қосуды көрсету үшін `artifacts/devportal/sorafs/latest` жаңартады.

Мысал (Sigstore + PoR бар толық құбыр):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Білуге тұрарлық жалаулар:

- `--out <dir>` – артефакт түбірін қайта анықтау (әдепкі уақыт белгісі бар қалталарды сақтайды).
- `--skip-build` – бар `docs/portal/build` қайта пайдалану (CI мүмкін болмаған кезде ыңғайлы
  желіден тыс айналар арқасында қайта құру).
- `--skip-sync-openapi` – `cargo xtask openapi` кезінде `npm run sync-openapi` өткізіп жіберіңіз
  crates.io-ға қол жеткізу мүмкін емес.
- `--skip-sbom` – екілік орнатылмаған кезде `syft` қоңырауын болдырмау (
  скрипт оның орнына ескертуді басып шығарады).
- `--proof` – әрбір CAR/манифест жұбы үшін `sorafs_cli proof verify` іске қосыңыз. көп-
  файлдың пайдалы жүктемелері әлі де CLI жүйесінде бөліктік жоспарды қолдауды қажет етеді, сондықтан бұл жалаушаны қалдырыңыз
  `plan chunk count` қателерін басып, бір рет қолмен растасаңыз, орнатудан босатылады.
  жоғары ағындағы қақпа жерлері.
- `--sign` – `sorafs_cli manifest sign` шақыру. Белгішені көрсетіңіз
  `SIGSTORE_ID_TOKEN` (немесе `--sigstore-token-env`) немесе CLI-ге оны пайдаланып алуға рұқсат етіңіз
  `--sigstore-provider/--sigstore-audience`.

Өндірістік артефактілерді жөнелту кезінде `docs/portal/scripts/sorafs-pin-release.sh` пайдаланыңыз.
Ол енді порталды, OpenAPI және SBOM пайдалы жүктемелерін буып, әрбір манифестке қол қояды және
`portal.additional_assets.json` ішінде қосымша актив метадеректерін жазады. Көмекші
CI бумалаушысы пайдаланатын бірдей қосымша тұтқаларды және жаңасын түсінеді
`--openapi-*`, `--portal-sbom-*` және `--openapi-sbom-*` қосқыштары
артефакт үшін бүркеншік ат кортеждерін тағайындаңыз, арқылы SBOM көзін қайта анықтаңыз
`--openapi-sbom-source`, белгілі бір пайдалы жүктемелерді өткізіп жіберу (`--skip-openapi`/`--skip-sbom`),
және `--syft-bin` бар әдепкі емес `syft` екілік нұсқасын көрсетіңіз.

Сценарий іске қосылған әрбір пәрменді көрсетеді; журналды шығару билетіне көшіріңіз
`package_summary.json`-пен қатар, шолушылар CAR дайджесттерін, жоспарларын ажырата алады.
метадеректер және Sigstore бума хэштері арнайы қабықша шығысын нақтылаусыз.

## 9-қадам — Шлюз + SoraDNS тексеруі

Кесуді жарияламас бұрын, жаңа бүркеншік аттың SoraDNS арқылы шешілетінін дәлелдеңіз
шлюздер жаңа дәлелдер:

1. **Зонд қақпасын іске қосыңыз.** `ci/check_sorafs_gateway_probe.sh` жаттығулары
   `cargo xtask sorafs-gateway-probe` демонстрацияларға қарсы
   `fixtures/sorafs_gateway/probe_demo/`. Нақты орналастырулар үшін зондты бағыттаңыз
   мақсатты хост атауында:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Зонд `Sora-Name`, `Sora-Proof` және `Sora-Proof-Status` кодтарын шешеді.
   `docs/source/sorafs_alias_policy.md` және манифест дайджесті кезінде сәтсіздікке ұшырайды,
   TTL немесе GAR байламдарының дрейфі.

   Жеңіл нүктелік тексерулер үшін (мысалы, тек байлау бумасы болғанда
   өзгертілді), `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` іске қосыңыз.
   Көмекші түсірілген байланыстыру бумасын тексереді және босатуға ыңғайлы
   толық тексеру бұрғысының орнына міндетті растауды қажет ететін билеттер.

2. **Бұрғылау дәлелдерін түсіріңіз.** Операторлық бұрғылар немесе PagerDuty құрғақ жүрістері үшін орау
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario бар зонд
   devportal-rollout -- …`. Орауыш тақырыптарды/журналдарды астында сақтайды
   `artifacts/sorafs_gateway_probe/<stamp>/`, `ops/drill-log.md` жаңартулары және
   (міндетті емес) кері ілмектерді немесе PagerDuty пайдалы жүктемелерін іске қосады. Орнату
   `--host docs.sora` IP мекенжайын қатты кодтаудың орнына SoraDNS жолын тексеру үшін.3. **DNS байланыстыруларын тексеріңіз.** Басқару бүркеншік аттың дәлелін жариялағанда, жазыңыз
   зондта (`--gar`) сілтеме жасалған GAR файлын тауып, оны шығарылымға тіркеңіз
   дәлел. Резолютор иелері бірдей енгізуді көрсете алады
   `tools/soradns-resolver` кэштелген жазбалардың жаңа манифестті құрметтеуін қамтамасыз ету үшін.
   JSON тіркемес бұрын, іске қосыңыз
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   сондықтан детерминирленген хостты салыстыру, манифест метадеректері және телеметриялық белгілер
   желіден тыс расталған. Көмекші келесімен бірге `--json-out` қорытындысын шығара алады
   GAR қол қойды, сондықтан шолушылар екілік файлды ашпай-ақ тексерілетін дәлелдерге ие болады.
  Жаңа GAR жобасын жасаған кезде артықшылық беріңіз
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (манифест файлы болмаған кезде ғана `--manifest-cid <cid>` нұсқасына оралыңыз
  қол жетімді). Көмекші енді CID **және** BLAKE3 дайджестін тікелей мына жерден алады
  манифест JSON, бос орынды қысқартады, қайталанатын `--telemetry-label` қайталайды
  жалаушалар, белгілерді сұрыптайды және әдепкі CSP/HSTS/Permissions-Policy шығарады
  JSON жазбас бұрын үлгілерді орнатыңыз, осылайша пайдалы жүктеме болған кезде де детерминирленген болып қалады
  операторлар әртүрлі қабықшалардан белгілерді алады.

4. **Лақап ат көрсеткіштерін қараңыз.** `torii_sorafs_alias_cache_refresh_duration_ms` сақтаңыз
   және экранда `torii_sorafs_gateway_refusals_total{profile="docs"}`
   зонд жұмыс істейді; екі серия да диаграммада
   `dashboards/grafana/docs_portal.json`.

## 10-қадам — Мониторинг және дәлелдемелерді жинақтау

- **Бақылау тақталары.** `dashboards/grafana/docs_portal.json` экспорттау (порталдың SLOs),
  `dashboards/grafana/sorafs_gateway_observability.json` (шлюз кідірісі +
  денсаулықты растау) және `dashboards/grafana/sorafs_fetch_observability.json`
  (оркестрдің денсаулығы) әрбір шығарылым үшін. JSON экспорттарын тіркеңіз
  шолушылардың Prometheus сұрауларын қайталауы үшін босату билеті.
- **Мұрағаттарды тексеру.** `artifacts/sorafs_gateway_probe/<stamp>/` git-қосымшасында сақтаңыз
  немесе сіздің дәлелдер шелегі. Тексеру қорытындысын, тақырыптарды және PagerDuty қосыңыз
  телеметрия сценарийі арқылы түсірілген пайдалы жүктеме.
- **Шығарылым жинағы.** Порталды/SBOM/OpenAPI CAR қорытындыларын, манифестті сақтаңыз
  бумалар, Sigstore қолтаңбалары, `portal.pin.report.json`, Байқап көру журналдары және
  бір уақыт белгісі бар қалта астындағы есептерді сілтеме арқылы тексеру (мысалы,
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Бұрғылау журналы.** Зондтар бұрғылаудың бөлігі болған кезде рұқсат етіңіз
  `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` қосу
  сондықтан бірдей дәлелдер SNNet-5 хаос талабын қанағаттандырады.
- **Билет сілтемелері.** Grafana панелінің идентификаторларына немесе тіркелген PNG экспорттарына сілтеме жасаңыз.
  өзгерту билеті, зерттеу есеп жолымен бірге, сондықтан өзгерту-рецензенттер
  қабықшаға қол жеткізусіз SLO-ларды салыстыра алады.

## 11-қадам — Көп дереккөзді алу жаттығулары және табло дәлелдері

SoraFS сайтында жариялау енді көп дереккөзді алу дәлелдерін қажет етеді (DOCS-7/SF-6)
жоғарыдағы DNS/шлюз дәлелдерімен қатар. Манифестті бекіткеннен кейін:

1. **`sorafs_fetch` нұсқасын тікелей манифестке қарсы іске қосыңыз.** Бірдей жоспарды/манифестті пайдаланыңыз
   2–3-қадамдарда жасалған артефактілер және әрқайсысы үшін берілген шлюз тіркелгі деректері
   провайдер. Аудиторлар оркестрді қайта ойнатуы үшін әрбір нәтижені сақтау
   шешім жолы:

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

   - Алдымен манифестпен сілтеме жасалған провайдердің жарнамаларын алыңыз (мысалы
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     және оларды `--provider-advert name=path` арқылы өткізіңіз, осылайша табло мүмкін болады
     мүмкіндіктер терезелерін анықтаушы түрде бағалау. Қолдану
     `--allow-implicit-provider-metadata` **тек** қондырмаларды қайта ойнатқанда
     CI; өндірістік жаттығулар қол қойылған хабарландыруларға сілтеме жасауы керек
     түйреуіш.
   - Манифест қосымша аймақтарға сілтеме жасағанда, пәрменді қайталаңыз
     сәйкес провайдер әрбір кэш/бүркеншік аттың сәйкестігіне ие болатындай кортеждер жасайды
     артефакті алу.

2. **Шығыстарды мұрағаттау.** `scoreboard.json` дүкені,
   `providers.ndjson`, `fetch.json` және `chunk_receipts.ndjson`
   дәлелдеме қалтасын шығарыңыз. Бұл файлдар тең салмақты түсіреді, қайталап көріңіз
   бюджет, кідіріс EWMA және басқару пакеті міндетті түрде әр бөлікке түсетін түсімдер
   SF-7 үшін сақтаңыз.

3. **Телеметрияны жаңарту.** Алу шығыстарын **SoraFS Fetch ішіне импорттау
   Бақылау мүмкіндігі** бақылау тақтасы (`dashboards/grafana/sorafs_fetch_observability.json`),
   `torii_sorafs_fetch_duration_ms`/`_failures_total` және
   аномалияларға арналған провайдер ауқымы панельдері. Grafana панелінің суретін келесіге байланыстырыңыз
   билетті табло жолымен бірге босатыңыз.

4. **Ескерту ережелерін қолданыңыз.** `scripts/telemetry/test_sorafs_fetch_alerts.sh` іске қосыңыз
   шығарылымды жабу алдында Prometheus ескерту жинағын тексеру үшін. Тіркеу
   DOCS-7 шолушылары дүңгіршекті растауы үшін билетке промқұралдың шығуы
   және баяу провайдер ескертулері қарулы күйінде қалады.

5. **CI жүйесіне сым.** Портал түйреуішінің жұмыс процесі `sorafs_fetch` қадамын артта қалдырады.
   `perform_fetch_probe` кірісі; оны сахналау/өндірістік іске қосу үшін қосыңыз
   алу дәлелдері манифест бумасымен бірге нұсқаулықсыз жасалады
   араласу. Жергілікті жаттығулар экспорттау арқылы бірдей сценарийді қайта пайдалана алады
   шлюз таңбалауыштары және `PIN_FETCH_PROVIDERS` үтірмен бөлінгенге орнату
   провайдерлер тізімі.

## Көтермелеу, бақылау және кері қайтару

1. **Жарнама:** бөлек қойылым және өндіріс бүркеншік аттарын сақтаңыз. арқылы насихаттау
   `manifest submit` бірдей манифестпен/бумамен қайта іске қосу, ауыстыру
   Өндіріс бүркеншік атын көрсету үшін `--alias-namespace/--alias-name`. Бұл
   QA орнату пинін бекіткеннен кейін қайта құруды немесе жұмыстан шығуды болдырмайды.
2. **Мониторинг:** PIN-тізілімінің бақылау тақтасын импорттаңыз
   (`docs/source/grafana_sorafs_pin_registry.json`) плюс порталға тән
   зондтар (`docs/portal/docs/devportal/observability.md` қараңыз). Бақылау сомасы туралы ескерту
   дрейф, сәтсіз зондтар немесе қайталап көру ұштарын дәлелдеу.
3. **Кері қайтару:** қайтару, алдыңғы манифестті қайта жіберу (немесе өшіру
   ағымдағы бүркеншік ат) `sorafs_cli manifest submit --alias ... --retire` арқылы.
   Қайтару дәлелдері болуы үшін әрқашан соңғы белгілі жинақты және CAR қорытындысын сақтаңыз
   CI журналдары айналса, қайта жасалады.

## CI жұмыс процесі үлгісі

Сіздің құбырыңыз кем дегенде:

1. Build + lint (`npm ci`, `npm run build`, бақылау сомасын құру).
2. Бума (`car pack`) және манифесттерді есептеу.
3. Тапсырма ауқымындағы OIDC (`manifest sign`) таңбалауышын пайдаланып қол қойыңыз.
4. Тексеру үшін артефактілерді жүктеп салыңыз (CAR, манифест, бума, жоспар, қорытындылар).
5. PIN тізіліміне жіберіңіз:
   - Тарту сұраулары → `docs-preview.sora`.
   - Тегтер / қорғалған филиалдар → өндіріс бүркеншік атын жылжыту.
6. Шығу алдында зондтарды + дәлелдеу тексеру қақпаларын іске қосыңыз.

`.github/workflows/docs-portal-sorafs-pin.yml` осы қадамдардың барлығын біріктіреді
қолмен шығарылымдар үшін. Жұмыс барысы:

- порталды құрастырады/сынайды,
- құрастыруды `scripts/sorafs-pin-release.sh` арқылы буады,
- GitHub OIDC көмегімен манифест жинағына қол қояды/тексереді,
- артефактілер ретінде CAR/манифест/бума/жоспар/дәлелдеу қорытындыларын жүктеп салады және
- (міндетті емес) құпиялар болған кезде манифестті + міндетті бүркеншік атын жібереді.

Тапсырманы іске қоспас бұрын келесі репозитарий құпияларын/айнымалы мәндерін конфигурациялаңыз:

| Аты | Мақсаты |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | `/v2/sorafs/pin/register` ашатын Torii хосты. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Жіберулермен жазылған дәуір идентификаторы. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Манифестті жіберуге қол қоюшы орган. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` `true` болғанда манифестке байланыстырылған бүркеншік ат кортежі. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-кодталған бүркеншік атын дәлелдеу жинағы (міндетті емес; бүркеншік атпен байланыстыруды өткізіп жіберуді өткізіп жіберіңіз). |
| `DOCS_ANALYTICS_*` | Басқа жұмыс үрдістері арқылы қайта пайдаланылатын бар аналитика/зерттеу соңғы нүктелері. |

Actions UI арқылы жұмыс процесін іске қосыңыз:

1. `alias_label` (мысалы, `docs.sora.link`), қосымша `proposal_alias`,
   және қосымша `release_tag` қайта анықтау.
2. Torii-ке тимей-ақ артефактілерді жасау үшін `perform_submit` құсбелгісін қойыңыз.
   (құрғақ жұмыс үшін пайдалы) немесе оны конфигурацияланғандарға тікелей жариялау үшін қосыңыз
   бүркеншік ат.

`docs/source/sorafs_ci_templates.md` әлі де жалпы CI көмекшілерін құжаттайды
осы репозиторийден тыс жобалар, бірақ порталдың жұмыс үрдісін таңдау керек
күнделікті шығарылымдар үшін.

## Бақылау тізімі

- [ ] `npm run build`, `npm run test:*` және `npm run check:links` жасыл.
- [ ] Артефактілерде түсірілген `build/checksums.sha256` және `build/release.json`.
- [ ] CAR, жоспар, манифест және `artifacts/` астында жасалған қорытынды.
- [ ] Sigstore бумасы + журналдармен сақталған бөлек қолтаңба.
- [ ] `portal.manifest.submit.summary.json` және `portal.manifest.submit.response.json`
      жіберулер орын алған кезде түсіріледі.
- [ ] `portal.pin.report.json` (және қосымша `portal.pin.proposal.json`)
      CAR/манифест артефактілерімен бірге мұрағатталған.
- [ ] `proof verify` және `manifest verify-signature` журналдары мұрағатталды.
- [ ] Grafana бақылау тақталары жаңартылды + Байқау сынақтары сәтті аяқталды.
- [ ] Қайтару жазбалары (алдыңғы манифест идентификаторы + бүркеншік ат дайджест).
      босату билеті.