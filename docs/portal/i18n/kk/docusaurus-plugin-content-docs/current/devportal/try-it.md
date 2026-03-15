---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Құм жәшігін қолданып көріңіз

Әзірлеуші порталы Torii нөміріне қоңырау шалу үшін қосымша «Байқап көріңіз» консолін жібереді.
құжаттамадан шықпай соңғы нүктелер. Консоль сұрауларды жібереді
Браузерлер CORS шектеулерін әлі де айналып өтуі үшін жинақталған прокси арқылы
тарифтік шектеулерді орындау және аутентификация.

## Алғышарттар

- Node.js 18.18 немесе одан жаңа нұсқасы (портал құрастыру талаптарына сәйкес келеді)
- Torii кезеңдік ортасына желіге кіру
- Сіз жаттығуды жоспарлап отырған Torii маршруттарына қоңырау шала алатын жеткізуші токен

Барлық прокси конфигурациясы орта айнымалылары арқылы орындалады. Төмендегі кесте
ең маңызды түймелерді тізімдейді:

| Айнымалы | Мақсаты | Әдепкі |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii негізгі URL, ол прокси сұрауларды | **Міндетті** |
| `TRYIT_PROXY_LISTEN` | Жергілікті даму үшін мекенжайды тыңдау (`host:port` немесе `[ipv6]:port` пішімі) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Проксиді шақыруы мүмкін нүктелердің үтірмен бөлінген тізімі | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Идентификатор `X-TryIt-Client` ішінде әрбір жоғары ағындық сұрау | үшін орналастырылған `docs-portal` |
| `TRYIT_PROXY_BEARER` | Әдепкі жеткізуші таңбалауышы Torii | мекенжайына жіберілді _бос_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Соңғы пайдаланушыларға `X-TryIt-Auth` | арқылы өз таңбалауыштарын жеткізуге рұқсат беріңіз `0` |
| `TRYIT_PROXY_MAX_BODY` | Сұраныстың ең үлкен негізгі өлшемі (байт) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Миллисекундпен жоғары ағын күту уақыты | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Клиент IP үшін тарифтік терезеге рұқсат етілген сұраулар | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Жылдамдықты шектеуге арналған жылжымалы терезе (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus стиліндегі метриканың соңғы нүктесі (`host:port` немесе `[ipv6]:port`) үшін қосымша тыңдау мекенжайы | _бос (өшірілген)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP жолы метриканың соңғы нүктесі арқылы қызмет етеді | `/metrics` |

Прокси сонымен қатар `GET /healthz` көрсетеді, құрылымдық JSON қателерін қайтарады және
журнал шығысынан тасымалдаушы таңбалауыштарын өңдейді.

Docs пайдаланушыларына проксиді көрсету кезінде `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` қосыңыз, осылайша Swagger және
RapiDoc панельдері пайдаланушы берген жеткізуші таңбалауыштарын жібере алады. Прокси әлі де тарифтік шектеулерді қолданады,
тіркелгі деректерін өңдейді және сұраудың әдепкі таңбалауышты немесе әрбір сұрауды қайта анықтауды пайдаланғанын жазады.
`TRYIT_PROXY_CLIENT_ID` параметрін `X-TryIt-Client` ретінде жібергіңіз келетін жапсырмаға орнатыңыз
(әдепкі `docs-portal`). Прокси қоңырау шалушы берген ақпаратты қиып, тексереді
`X-TryIt-Client` мәндері, осы әдепкіге оралады, осылайша шлюздерді реттей алады
шолғыш метадеректерін корреляциясыз тексеру.

## Проксиді жергілікті түрде іске қосыңыз

Порталды бірінші рет орнатқанда тәуелділіктерді орнатыңыз:

```bash
cd docs/portal
npm install
```

Проксиді іске қосыңыз және оны Torii данасына бағыттаңыз:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Скрипт байланыстырылған мекенжайды тіркейді және сұрауларды `/proxy/*` мекенжайына жібереді.
конфигурацияланған Torii бастапқы.

Розетканы байланыстырмас бұрын сценарий оны растайды
`static/openapi/torii.json` жазылған дайджестке сәйкес келеді
`static/openapi/manifest.json`. Егер файлдар дрейф болса, пәрмен арқылы шығады
қатені береді және `npm run sync-openapi -- --latest` іске қосуды нұсқайды. Экспорттау
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` тек төтенше жағдайларды жою үшін; прокси болады
техникалық қызмет көрсету терезелері кезінде қалпына келтіру үшін ескертуді жазып, жалғастырыңыз.

## Портал виджеттерін өткізіңіз

Әзірлеуші порталын құрастырғанда немесе қызмет еткенде, виджеттердің URL мекенжайын орнатыңыз
прокси үшін пайдалану керек:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Келесі компоненттер `docusaurus.config.js` мына мәндерді оқиды:

- **Swagger UI** — `/reference/torii-swagger` мекенжайында көрсетілген; алдын ала рұқсат береді
  таңбалауыш болған кезде ұсынушы схемасы, `X-TryIt-Client` бар сұрауларды тегтер,
  `X-TryIt-Auth` енгізеді және прокси арқылы қоңырауларды қайта жазады.
  `TRYIT_PROXY_PUBLIC_URL` орнатылды.
- **RapiDoc** — `/reference/torii-rapidoc` мекенжайында көрсетілген; таңбалауыш өрісін көрсетеді,
  Swagger панелімен бірдей тақырыптарды қайта пайдаланады және проксиге бағыттайды
  URL конфигурацияланған кезде автоматты түрде.
- **Try it console** — API шолу бетіне ендірілген; тапсырысты жіберуге мүмкіндік береді
  сұрауларды, тақырыптарды қарау және жауап беру органдарын тексеру.

Екі панельде де оқитын **суретті таңдау құралы** бар
`docs/portal/static/openapi/versions.json`. Бұл индексті толтырыңыз
`npm run sync-openapi -- --version=<label> --mirror=current --latest` сондықтан
шолушылар тарихи сипаттамалар арасында өтіп, жазылған SHA-256 дайджестін көре алады,
және қолданбастан бұрын шығарылым суретінің қол қойылған манифесті бар-жоғын растаңыз
интерактивті виджеттер.

Кез келген виджеттегі таңбалауышты өзгерту тек ағымдағы шолғыш сеансына әсер етеді; the
прокси ешқашан сақталмайды немесе берілген таңбалауышты журналға енгізбейді.

## Қысқа мерзімді OAuth таңбалауыштары

Ұзақ өмір сүретін Torii таңбалауыштарын шолушыларға таратпау үшін, Байқап көріңіз.
OAuth серверіне консоль. Төмендегі орта айнымалылары болған кезде
портал құрылғы кодының кіру виджетін көрсетеді, қысқа мерзімді тасымалдаушы белгілерді шығарады,
және оларды консоль пішініне автоматты түрде енгізеді.

| Айнымалы | Мақсаты | Әдепкі |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth құрылғы авторизациясының соңғы нүктесі (`/oauth/device/code`) | _бос (өшірілген)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` | қабылдайтын таңбалауыш соңғы нүкте _бос_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth клиент идентификаторы құжаттарды алдын ала қарау үшін тіркелген | _бос_ |
| `DOCS_OAUTH_SCOPE` | Жүйеге кіру кезінде сұралған кеңістікпен бөлінген аумақтар | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Токенді |мен байланыстыру үшін қосымша API аудиториясы _бос_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Бекітуді күту кезіндегі сауалнаманың ең аз аралығы (мс) | `5000` (<5000мс мәндер қабылданбайды) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Қайтарылған құрылғы кодының жарамдылық мерзімі аяқталатын терезе (секундтар) | `600` (300 және 900 арасында қалуы керек) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Қайта қол жетімділік белгісінің қызмет ету мерзімі (секундтар) | `900` (300 және 900 арасында қалуы керек) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth қолдануды әдейі өткізіп жіберетін жергілікті алдын ала қараулар үшін `1` күйіне орнатыңыз | _орнатылмаған_ |

Мысал конфигурация:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

`npm run start` немесе `npm run build` іске қосқан кезде портал осы мәндерді ендіреді.
`docusaurus.config.js` ішінде. Жергілікті алдын ала қарау кезінде Байқап көру картасы а көрсетеді
«Құрылғы кодымен кіру» түймесі. Пайдаланушылар көрсетілген кодты OAuth жүйесіне енгізеді
тексеру парағы; құрылғы ағыны виджетті сәтті орындағаннан кейін:

- шығарылған тасымалдаушы таңбалауышын «Тарау» консолі өрісіне енгізеді,
- сұраныстарды бар `X-TryIt-Client` және `X-TryIt-Auth` тақырыптарымен белгілейді,
- қалған қызмет мерзімін көрсетеді және
- таңбалауыш мерзімі біткен кезде автоматты түрде тазалайды.

Қолмен Bearer енгізуі қолжетімді болып қалады — кез келген уақытта OAuth айнымалы мәндерін өткізіп жіберіңіз
шолушыларды уақытша таңбалауышты өздері қоюға немесе экспорттауға мәжбүрлеуді қалайды
`DOCS_OAUTH_ALLOW_INSECURE=1` анонимді кіруге болатын оқшауланған жергілікті алдын ала қараулар үшін
қолайлы. OAuth конфигурацияланбаған құрастырулар енді талаптарды қанағаттандыра алмайды
DOCS-1b жол картасы қақпасы.

📌 [Қауіпсіздікті қатайту және қалам сынау бақылау тізімін] (./security-hardening.md) қарап шығыңыз.
порталды зертхананың сыртына шығарар алдында; ол қауіп моделін құжаттайды,
CSP/Сенімді түрлер профилі және енді DOCS-1b жабатын ену сынағы қадамдары.

## Norito-RPC үлгілері

Norito-RPC сұраулары JSON маршруттары сияқты бірдей прокси мен OAuth сантехникасын ортақ пайдаланады,
олар жай ғана `Content-Type: application/x-norito` орнатып, жібереді
NRPC спецификациясында сипатталған алдын ала кодталған Norito пайдалы жүктемесі
(`docs/source/torii/nrpc_spec.md`).
Репозиторий `fixtures/norito_rpc/` астында канондық пайдалы жүктемелерді жібереді, сондықтан портал
авторлар, SDK иелері және шолушылар CI пайдаланатын нақты байтты қайталай алады.

### Байқап көру консолінен Norito пайдалы жүктемесін жіберу

1. `fixtures/norito_rpc/transfer_asset.norito` сияқты арматураны таңдаңыз. Бұл
   файлдар өңделмеген Norito конверттері; base64-кодтамаңыз**.
2. Swagger немесе RapiDoc ішінде NRPC соңғы нүктесін табыңыз (мысалы
   `POST /v1/pipeline/submit`) және **Content-Type** селекторын келесіге ауыстырыңыз
   `application/x-norito`.
3. Сұрау мәтінінің редакторын **екілік** күйіне ауыстырыңыз (Swagger «Файл» режимі немесе
   RapiDoc «Екілік/Файл» селекторы) және `.norito` файлын жүктеңіз. Виджет
   прокси арқылы байттарды өзгертусіз жібереді.
4. Өтінішті жіберіңіз. Егер Torii `X-Iroha-Error-Code: schema_mismatch` қайтарса,
   екілік пайдалы жүктемелерді қабылдайтын соңғы нүктеге қоңырау шалып жатқаныңызды және
   `fixtures/norito_rpc/schema_hashes.json` ішінде жазылған схема хэшін растаңыз
   сіз соғып жатқан Torii құрылымына сәйкес келеді.

Консоль ең соңғы файлды жадта сақтайды, осылайша сіз оны қайта жібере аласыз
әртүрлі авторизация белгілерін немесе Torii хосттарын орындау кезінде пайдалы жүктеме. Қосу
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` сіздің жұмыс үрдісіңізге шығарады
NRPC-4 қабылдау жоспарында сілтеме жасалған дәлелдер жинағы (журнал + JSON қорытындысы),
шолу кезінде «Байқап көріңіз» жауабының скриншотымен тамаша үйлеседі.

### CLI мысалы (бұралу)

Дәл осындай құрылғыларды `curl` арқылы порталдан тыс қайта ойнатуға болады, бұл пайдалы
проксиді тексеру немесе шлюз жауаптарын жөндеу кезінде:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

Арматураны `transaction_fixtures.manifest.json` тізімінде көрсетілген кез келген жазбаға ауыстырыңыз
немесе `cargo xtask norito-rpc-fixtures` көмегімен өзіңіздің пайдалы жүктемеңізді кодтаңыз. Torii кезінде
canary режимінде болса, `curl` прокси-серверіне көрсетуге болады
(`https://docs.sora.example/proxy/v1/pipeline/submit`) бірдей жаттығу
портал виджеттері пайдаланатын инфрақұрылым.

## Бақылану және операцияларӘрбір сұрау әдісі, жолы, шығу тегі, жоғары ағын күйі және
аутентификация көзі (`override`, `default` немесе `client`). Токендер ешқашан болмайды
сақталады — тасымалдаушы тақырыптары мен `X-TryIt-Auth` мәндері бұрын өңделеді
журнал жүргізу — сондықтан сіз алаңдамай stdout-ты орталық коллекторға жібере аласыз
құпиялар ағып жатыр.

### Денсаулықты тексеру және ескерту

Орналастыру кезінде немесе кесте бойынша жинақталған зондты іске қосыңыз:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Қоршаған ортаны қорғау түймелері:

- `TRYIT_PROXY_SAMPLE_PATH` — жаттығуға қосымша Torii бағыты (`/proxy` жоқ).
- `TRYIT_PROXY_SAMPLE_METHOD` — әдепкі бойынша `GET`; жазу маршруттары үшін `POST` мәніне орнатыңыз.
- `TRYIT_PROXY_PROBE_TOKEN` — үлгі қоңырауы үшін уақытша жеткізуші таңбалауышын енгізеді.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — әдепкі 5s күту уақытын қайта анықтайды.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` үшін қосымша Prometheus мәтіндік файл тағайындау орны.
- `TRYIT_PROXY_PROBE_LABELS` — көрсеткіштерге қосылған үтірмен бөлінген `key=value` жұптары (әдепкі `job=tryit-proxy` және `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` — `TRYIT_PROXY_METRICS_LISTEN` қосылған кезде сәтті жауап беруі керек қосымша көрсеткіштердің соңғы нүктесі URL мекенжайы (мысалы, `http://localhost:9798/metrics`).

Зондты жазылатын файлға бағыттау арқылы нәтижелерді мәтіндік файл жинағышына беріңіз
жол (мысалы, `/var/lib/node_exporter/textfile_collector/tryit.prom`) және
кез келген теңшелетін белгілерді қосу:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

Сценарий метрикалық файлды атомдық түрде қайта жазады, осылайша коллектор әрқашан a оқиды
толық жүктеме.

`TRYIT_PROXY_METRICS_LISTEN` конфигурацияланған кезде, орнатыңыз
`TRYIT_PROXY_PROBE_METRICS_URL` метриканың соңғы нүктесіне, сондықтан зонд тез істен шығады
сыдырма беті жоғалып кетсе (мысалы, қате конфигурацияланған кіріс немесе жоқ
брандмауэр ережелері). Әдеттегі өндірістік параметр болып табылады
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Жеңіл ескерту үшін зондты бақылау стекіне жалғаңыз. A Prometheus
Мысал екі қатарынан сәтсіздіктен кейінгі беттер:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Көрсеткіштердің соңғы нүктесі және бақылау тақталары

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (немесе кез келген хост/порт жұбы) алдында орнатыңыз
Prometheus пішімделген метриканың соңғы нүктесін көрсету үшін проксиді іске қосу. Жол
әдепкі бойынша `/metrics`, бірақ арқылы қайта анықтауға болады
`TRYIT_PROXY_METRICS_PATH=/custom`. Әрбір сызу әр әдіс үшін есептегіштерді қайтарады
сұрау қорытындылары, мөлшерлеме шегінен бас тарту, жоғары ағындағы қателер/тайм-ауттар, прокси нәтижелері,
және кешігу туралы қорытындылар:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP коллекторларын метриканың соңғы нүктесіне бағыттаңыз және
бар `dashboards/grafana/docs_portal.json` панельдері SRE құйрықты бақылай алады
кідіріс және журналдарды талдаусыз қабылдамау өсінділері. Прокси автоматты түрде
операторларға қайта іске қосуды анықтауға көмектесу үшін `tryit_proxy_start_timestamp_ms` жариялайды.

### Қайтаруды автоматтандыру

Мақсатты Torii URL мекенжайын жаңарту немесе қалпына келтіру үшін басқару көмекшісін пайдаланыңыз. Сценарий
алдыңғы конфигурацияны `.env.tryit-proxy.bak` ішінде сақтайды, сондықтан кері қайтарулар
жалғыз команда.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Қолдануыңыз болса, `--env` немесе `TRYIT_PROXY_ENV` көмегімен env файлының жолын қайта анықтаңыз
конфигурацияны басқа жерде сақтайды.