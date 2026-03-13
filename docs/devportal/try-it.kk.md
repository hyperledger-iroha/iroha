---
lang: kk
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

Әзірлеуші ​​порталы Torii REST API үшін "Байқап көру" консолін жібереді. Бұл нұсқаулық
қолдау көрсететін проксиді іске қосу және консольді кезеңге қосу жолын түсіндіреді
тіркелгі деректерін ашпастан шлюз.

## Алғышарттар

- Iroha репозиторийін тексеру (жұмыс кеңістігінің түбірі).
- Node.js 18.18+ (порталдың негізгі деңгейіне сәйкес келеді).
- Torii соңғы нүктеге жұмыс станциясынан қол жеткізуге болады (кезеңдік немесе жергілікті).

## 1. OpenAPI суретін жасаңыз (қосымша)

Консоль порталдың анықтамалық беттері сияқты бірдей OpenAPI пайдалы жүктемесін қайта пайдаланады. Егер
Torii бағыттарын өзгерттіңіз, суретті қайта жасаңыз:

```bash
cargo xtask openapi
```

Тапсырма `docs/portal/static/openapi/torii.json` деп жазады.

## 2. Байқап көру проксиін іске қосыңыз

Репозиторий түбірінен:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Қоршаған ортаның айнымалылары

| Айнымалы | Сипаттама |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii негізгі URL (міндетті). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Проксиді пайдалануға рұқсат етілген түпнұсқалардың үтірмен бөлінген тізімі (әдепкі бойынша `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Қосымша әдепкі жеткізуші таңбалауышы барлық проксилік сұрауларға қолданылады. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Қоңырау шалушының `Authorization` тақырыбын сөзбе-сөз бағыттау үшін `1` мәніне орнатыңыз. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Жад жылдамдығын шектегіш параметрлері (әдепкі: 60 секунд сайын 60 сұрау). |
| `TRYIT_PROXY_MAX_BODY` | Максималды сұраудың пайдалы жүктемесі қабылданды (байт, әдепкі 1МБ). |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii сұраулары үшін жоғары ағын күту уақыты (әдепкі 10000 мс). |

Прокси мыналарды көрсетеді:

- `GET /healthz` — дайындықты тексеру.
- `/proxy/*` — жолды және сұрау жолын сақтайтын проксилік сұраулар.

## 3. Порталды іске қосыңыз

Жеке терминалда:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` сайтына кіріп, Көріп көру консолін пайдаланыңыз. Дәл солай
ортаның айнымалы мәндері Swagger UI және RapiDoc ендірулерін конфигурациялайды.

## 4. Бірлік сынақтарын орындау

Прокси түйінге негізделген жылдам сынақ жиынтығын көрсетеді:

```bash
npm run test:tryit-proxy
```

Сынақтар мекенжайды талдауды, бастапқы өңдеуді, жылдамдықты шектеуді және тасымалдаушыны қамтиды
инъекция.

## 5. Зондтарды автоматтандыру және метрика

`/healthz` және үлгі соңғы нүктені тексеру үшін жинақталған зондты пайдаланыңыз:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Қоршаған ортаны қорғау түймелері:

- `TRYIT_PROXY_SAMPLE_PATH` — жаттығу үшін қосымша Torii бағыты (`/proxy` жоқ).
- `TRYIT_PROXY_SAMPLE_METHOD` — әдепкі бойынша `GET`; жазу маршруттары үшін `POST` мәніне орнатыңыз.
- `TRYIT_PROXY_PROBE_TOKEN` — үлгі қоңырауы үшін уақытша жеткізуші таңбалауышын енгізеді.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — әдепкі 5s күту уақытын қайта анықтайды.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` үшін Prometheus мәтіндік файлының тағайындалған орны.
- `TRYIT_PROXY_PROBE_LABELS` — көрсеткіштерге қосылған үтірмен бөлінген `key=value` жұптары (әдепкі `job=tryit-proxy` және `instance=<proxy URL>`).

`TRYIT_PROXY_PROBE_METRICS_FILE` орнатылған кезде, сценарий файлды қайта жазады
атомдық түрде сіздің node_exporter/мәтіндік файл жинағышыңыз әрқашан толықты көреді
пайдалы жүк. Мысалы:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Алынған көрсеткіштерді Prometheus мекенжайына жіберіңіз және үлгі ескертуін келесіде қайта пайдаланыңыз.
әзірлеуші-портал құжаттары `probe_success` `0` мәніне төмендеген кезде бетке.

## 6. Өндірістің қатаюының бақылау парағы

Жергілікті дамудан тыс проксиді жарияламас бұрын:

- TLS-ті проксиден бұрын аяқтаңыз (кері прокси немесе басқарылатын шлюз).
- Құрылымдық журналды конфигурациялау және бақылау құбырларына жіберу.
- Тасымалдаушы белгілерді айналдырыңыз және оларды құпия менеджерде сақтаңыз.
- Проксидің `/healthz` соңғы нүктесін және жиынтық кідіріс көрсеткіштерін бақылаңыз.
- Тарифтік шектеулерді Torii кезеңдік квоталарыңызбен теңестіріңіз; `Retry-After` реттеңіз
  клиенттерге дроссельді хабарлау үшін мінез-құлық.