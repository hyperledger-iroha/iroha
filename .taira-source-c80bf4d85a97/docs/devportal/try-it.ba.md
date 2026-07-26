---
lang: ba
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

Төҙөүсе порталы “Уны һынап ҡарағыҙ” консоль өсөн I18NT0000000005X REST API. Был ҡулланма
аңлата, нисек эшләтеп ебәрергә терәк прокси һәм консоль тоташтырыу өсөн сәхнәләштереү
шлюз ышаныс ҡағыҙҙарын фашламай.

## Алдан шарттар

- I18NT000000004X һаҡлағыс касса (эш урыны тамыры).
- Node.js 18.18+ (порталь база һыҙығына тап килә).
- I18NT000000006X осона тиклем һеҙҙең эш станцияһынан (стужнеция йәки урындағы) етергә мөмкин.

## 1. I18NT000000002X снимок генерациялау (теләк)

Консоль порталь белешмә биттәре кеүек үк I18NT0000000003X файҙалы йөктө ҡабаттан ҡуллана. Әгәр
һеҙ үҙгәрткән I18NT0000000007X маршруттар, регенерация снимок:

```bash
cargo xtask openapi
```

Бурыс `docs/portal/static/openapi/torii.json` яҙа.

## 2. Башлағыҙ, уны һынап ҡарағыҙ прокси

Репозиторий тамырынан:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Тирә-яҡ мөхит үҙгәртеүселәре

| Үҙгәртеүсән | Тасуирлама |
|---------|--------------|
| `TRYIT_PROXY_TARGET` | Torii база URL (кәрәкле). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Комма менән айырылған сығыш исемлеге прокси ҡулланырға мөмкинлек бирҙе (поктиллиҙар `http://localhost:3000` тиклем). |
| `TRYIT_PROXY_BEARER` | Ҡулланылған бөтә прокси-проксилы үтенестәргә ҡағылышлы несущий токен. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | `1`-ҡа тиклем йыйылмаһы шылтыратыусы I18NI000000027X баш һүҙмә-һүҙен тапшырыу өсөн. |
| `TRYIT_PROXY_RATE_LIMIT` / I18NI000000029X X | Хәтерҙәге ставкаларҙы сикләүсе параметрҙар (поктынь: 60-сы 60-сыға запрос). |
| I18NI000000030X | Максималь запрос файҙалы йөк ҡабул ителә (байттар, ғәҙәттәгесә 1МиБ). |
| `TRYIT_PROXY_TIMEOUT_MS` | Өҫкө ағым тайм-аут өсөн I18NT0000000009X үтенестәр (дефолт 10000мс). |

Прокси фашлай:

- `GET /healthz` — әҙерлек тикшерелеүе.
- `/proxy/*` — прокси-запростар, юлды һәм эҙләү ептәрен һаҡлау.

## 3. Порталды эшләтеп ебәрегеҙ

Айырым терминалда:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview`-ҡа инергә һәм Try It консоль ҡулланыу. Шул уҡ .
тирә-яҡ мөхит үҙгәртеүселәре конфигурациялау Swagger UI һәм RapiDoc встраиваемый.

## 4. Йүгереп берәмеге һынауҙары

Прокси тиҙ Node-нигеҙендә һынау люкс фашлай:

```bash
npm run test:tryit-proxy
```

Һынауҙар ҡаплай адресы анализлау, сығыштар менән эш итеү, ставка сикләү, һәм йөрөтөүсе .
инъекция.

## 5. Зонд автоматлаштырыу & метрика

Ҡулланыу зонд раҫлау өсөн I18NI0000000035X һәм өлгө ос нөктәһе:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Тирә-яҡ мөхит ручкалары:

- I18NI000000036X — опциональ I18NT0000000010X маршруты (I18NI000000037X булмаһа) күнекмәләр өсөн.
- I18NI000000038X — I18NI000000039X тиклем ғәҙәттәгесә; яҙыу маршруттары өсөн I18NI000000040Хҡа ҡуйылған.
- `TRYIT_PROXY_PROBE_TOKEN` — өлгө шылтыратыу өсөн ваҡытлыса йөрөтөүсе токен индерә.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 5s тайм-аут ғәҙәттәгесә өҫтөнлөк итә.
- I18NI000000043X — I18NI000000000044X/`probe_duration_seconds` өсөн Prometheus.
- I18NI000000046X — воезнака I18NI0000000047X парҙары метрикаға ҡушылған (I18NI000000048X һәм I18NI000000049X X).

Ҡасан I18NI000000050X ҡуйылған, сценарий файлды яңынан яҙа .
атомлы шулай һеҙҙең node_exporter/текстфайл коллекционер һәр ваҡыт тулы күрә
файҙалы йөк. Миҫал:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Һөҙөмтәлә барлыҡҡа килгән метрикаларҙы I18NT0000000001X тиклем алға һәм өлгө иҫкәртмәһен ҡабаттан ҡулланыу.
портал docs биткә ҡасан I18NI000000051X төшөп I18NI000000052X.

## 6. Производство ҡатыу тикшерелгән исемлек

Урындағы үҫештән тыш проксины баҫтырыр алдынан:

- ТЛС-ты прокси-серь алдынан туҡтатығыҙ (кире прокси йәки идара итеү шлюз).
- Конфигурациялау структуралы логин һәм алға күҙәтеүсәнлек торбалары.
- ротаж йөрөтөүсе жетон һәм уларҙы һаҡлау һеҙҙең серҙәр менеджеры.
- Монитор прокси’s `/healthz` ос нөктәһе һәм агрегат латентлыҡ метрикаһы.
- Һеҙҙең Torii стадияһында квоталар менән ставка сикләүҙәре; I18NI000000054X көйләү
  тәртибе клиенттарға дроссель менән аралашыу өсөн.