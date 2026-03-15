---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Құм жәшігін қолданып көріңіз

Әзірлеуші порталы Torii нөміріне қоңырау шалу үшін қосымша «Байқап көріңіз» консолін жібереді.
құжаттамадан шықпай соңғы нүктелер. Консоль сұрауларды жібереді
Браузерлер CORS шектеулерін әлі де айналып өтуі үшін жинақталған прокси арқылы
тарифтік шектеулерді орындау және аутентификация.

## Алғышарттар

- Node.js 18.18 немесе одан жаңа нұсқасы (портал құрастыру талаптарына сәйкес келеді)
- Torii сахналау ортасына желіге кіру
- Сіз жаттығуды жоспарлап отырған Torii маршруттарына қоңырау шала алатын жеткізуші токен

Барлық прокси конфигурациясы орта айнымалылары арқылы орындалады. Төмендегі кесте
ең маңызды түймелерді тізімдейді:

| Айнымалы | Мақсаты | Әдепкі |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii негізгі URL, ол прокси сұрауларды | **Міндетті** |
| `TRYIT_PROXY_LISTEN` | Жергілікті даму үшін мекенжайды тыңдау (`host:port` немесе `[ipv6]:port` пішімі) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Проксиді шақыруы мүмкін нүктелердің үтірмен бөлінген тізімі | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | Әдепкі жеткізуші таңбалауышы Torii | мекенжайына жіберілді _бос_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Соңғы пайдаланушыларға `X-TryIt-Auth` | арқылы өз таңбалауыштарын жеткізуге рұқсат беріңіз `0` |
| `TRYIT_PROXY_MAX_BODY` | Сұраныстың ең үлкен негізгі өлшемі (байт) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Миллисекундпен жоғары ағын күту уақыты | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Клиент IP үшін тарифтік терезеге рұқсат етілген сұраулар | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Жылдамдықты шектеуге арналған жылжымалы терезе (мс) | `60000` |

Прокси сонымен қатар `GET /healthz` көрсетеді, құрылымдық JSON қателерін қайтарады және
журнал шығысынан тасымалдаушы таңбалауыштарын өңдейді.

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

## Портал виджеттерін өткізіңіз

Әзірлеуші порталын құрастырғанда немесе қызмет еткенде, виджеттердің URL мекенжайын орнатыңыз
прокси үшін пайдалану керек:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Келесі компоненттер `docusaurus.config.js` ішінен осы мәндерді оқиды:

- **Swagger UI** — `/reference/torii-swagger` мекенжайында көрсетілген; сұрауды пайдаланады
  тасымалдаушы таңбалауыштарын автоматты түрде тіркейтін тосқауыл.
- **RapiDoc** — `/reference/torii-rapidoc` мекенжайында көрсетілген; таңбалауыш өрісін көрсетеді
  және проксиге қарсы сынап көру сұрауларына қолдау көрсетеді.
- **Try it console** — API шолу бетіне ендірілген; тапсырысты жіберуге мүмкіндік береді
  сұрауларды, тақырыптарды қарау және жауап беру органдарын тексеру.

Кез келген виджеттегі таңбалауышты өзгерту тек ағымдағы шолғыш сеансына әсер етеді; the
прокси ешқашан сақталмайды немесе берілген таңбалауышты журналға енгізбейді.

## Бақылану және операциялар

Әрбір сұрау әдісі, жолы, шығу тегі, жоғары ағын күйі және
аутентификация көзі (`override`, `default` немесе `client`). Токендер ешқашан болмайды
сақталады — тасымалдаушы тақырыптары мен `X-TryIt-Auth` мәндері бұрын өңделеді
журнал жүргізу — сондықтан сіз алаңдамай stdout-ты орталық коллекторға жібере аласыз
құпиялар ағып жатыр.

### Денсаулықты тексеру және ескертуОрналастыру кезінде немесе кесте бойынша жинақталған зондты іске қосыңыз:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Қоршаған ортаны қорғау түймелері:

- `TRYIT_PROXY_SAMPLE_PATH` — жаттығу үшін қосымша Torii бағыты (`/proxy` жоқ).
- `TRYIT_PROXY_SAMPLE_METHOD` — әдепкі бойынша `GET`; жазу маршруттары үшін `POST` мәніне орнатыңыз.
- `TRYIT_PROXY_PROBE_TOKEN` — үлгі қоңырауы үшін уақытша жеткізуші таңбалауышын енгізеді.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — әдепкі 5s күту уақытын қайта анықтайды.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` үшін қосымша Prometheus мәтіндік файл тағайындау орны.
- `TRYIT_PROXY_PROBE_LABELS` — көрсеткіштерге қосылған үтірмен бөлінген `key=value` жұптары (әдепкі `job=tryit-proxy` және `instance=<proxy URL>`).

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

Env файл жолын `--env` немесе `TRYIT_PROXY_ENV` арқылы ауыстырыңыз
конфигурацияны басқа жерде сақтайды.