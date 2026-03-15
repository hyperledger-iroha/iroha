---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ул ҡом йәшниктәрен һынап ҡарағыҙ

Төҙөүсе порталы караптары факультатив “Бына уны һынап ҡарағыҙ” консоль, шулай итеп, һеҙ шылтырата аласыз Torii .
документациянан сыҡмайынса ла ос нөктәләре. Консоль релелар үтенестәре
аша йыйып алынған прокси шулай браузерҙар урап үтергә мөмкин CORS сиктәрен, ә әле лә
ставка сиктәре һәм аутентификацияны үтәү.

## Алдан шарттар

- Node.js 18.18 йәки яңы (портал төҙөү талаптарына тап килә)
- Селтәр инеү Torii стажировка мөхитенә
- Һеҙҙең менән шөғөлләнергә планлаштырған Torii маршруттарына шылтырата алған несущий токен

Бөтә прокси конфигурацияһы тирә-яҡ мөхит үҙгәртеүселәре аша эшләнә. Түбәндәге таблица
иң мөһим ручкалар исемлеге:

| Үҙгәртеүсән | Маҡсат | Ғәҙәттәгесә |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | База Torii URL, тип прокси-форвардтар үтенестәре | **Кәрәк** |
| `TRYIT_PROXY_LISTEN` | Урындағы үҫеш өсөн тыңлау адресы (формат `host:port` йәки `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Комма-айырылған исемлек сығышы, тип атау мөмкин прокси | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | Ғәҙәттәгесә йөрөтөүсе токен Torii тиклем ебәрелгән | _бут_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Ҡулланыу рөхсәт итеү өсөн аҙаҡҡы ҡулланыусылар үҙ токен аша тәьмин итеү өсөн `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максималь үтенес тән ҙурлығы (байттар) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Миллисекундтарҙа өҫкө ағым тайм-аут | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Запростар рөхсәт ителә ставка тәҙрә клиент IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Слайд тәҙрә өсөн ставка сикләү (мс) | `60000` |

Прокси шулай уҡ `GET /healthz` X, структуралы JSON хаталарын ҡайтара, һәм
1990 йылдарҙа был йүнәлештәге эштәрҙе логин сығарыуҙан реконструкциялай.

## локаль прокси-прокси-сервис башлағыҙ

Зависимость монтаж беренсе тапҡыр һеҙ портал ҡуйҙы:

```bash
cd docs/portal
npm install
```

Һеҙҙең Torii экземплярында прокси һәм уны күрһәтегеҙ:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Сценарийҙа бәйләнгән адрес һәм форвардтар `/proxy/*` XX .
конфигурацияланған Torii сығышы.

## Сым портал виджеттары

Ҡасан һеҙ төҙөү йәки хеҙмәтләндерергә портал төҙөүсе, URL-адрес, тип виджеттар ҡуйырға .
прокси өсөн ҡулланырға тейеш:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Түбәндәге компоненттар был ҡиммәттәрҙе `docusaurus.config.js`-тан уҡый:

- **Сваггер UI** — `/reference/torii-swagger`-та күрһәтелгән; үтенес ҡуллана
  перехватчик беркетергә несущий жетон автоматик.
- **РапиДок** — `/reference/torii-rapidoc`-та күрһәтелгән; жетон яланын көҙгөләй
  һәм тырнаҡ-ул үтенестәрен яҡлай ҡаршы прокси.
- **Уны консоль** һынап ҡарағыҙ — API дөйөм ҡараш битендә индерелгән; һеҙгә заказ бирергә мөмкинлек бирә
  үтенестәр, башлыҡтарҙы ҡарау һәм яуап органдарын тикшерергә.

Теләһә ниндәй виджетта үҙгәртеү тик хәҙерге браузер сессияһына йоғонто яһай; был
прокси бер ҡасан да һаҡланмай йәки логин тәьмин ителгән токен.

## Күҙәтеүсәнлек һәм операциялар

Һәр үтенес бер тапҡыр ысул, юл, сығышы, өҫкө ағымдағы статусы һәм
аутентификация сығанағы (`override`, `default`, йәки `client`). Токендар бер ҡасан да түгел
һаҡланған — йөрөтөүсе башлыҡтары ла, `X-TryIt-Auth` ҡиммәттәре лә 2012 йылға тиклем үҙгәртелә.
логин-шулай итеп, һеҙ алға stdout үҙәк коллекционер тураһында борсолмай
серҙәре ағып сыға.

### Һаулыҡ зондтары & иҫкәртмәЙүгереп зонд ваҡытында йәки график буйынса таратыу:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Тирә-яҡ мөхит ручкалары:

- `TRYIT_PROXY_SAMPLE_PATH` — опциональ Torii маршруты (`/proxy` X) күнекмәләр өсөн.
- `TRYIT_PROXY_SAMPLE_METHOD` — `GET` тиклем ғәҙәттәгесә; яҙырға маршруттар өсөн `POST` ҡуйылған.
- `TRYIT_PROXY_PROBE_TOKEN` — өлгө шылтыратыу өсөн ваҡытлыса йөрөтөүсе токен индерә.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 5s тайм-аут ғәҙәттәгесә өҫтөнлөк итә.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — I18NNNNNNNNNNNNNNNita `probe_success`/`probe_duration_seconds` өсөн опциональ стаканлы йүнәлеш.
- `TRYIT_PROXY_PROBE_LABELS` — метрикаларға (`job=tryit-proxy` һәм `instance=<proxy URL>`-ҡа тиклем ғәҙәттәгесә ғәҙәттәгесә `key=value` парҙары бойоҡ.

Һөҙөмтәләрҙе зондты яҙма рәүештә күрһәтеп, текст файлы коллекторына туҡландырығыҙ
юл (мәҫәлән, `/var/lib/node_exporter/textfile_collector/tryit.prom`) һәм
теләһә ниндәй ҡулланыусылар өсөн ярлыҡтар өҫтәү:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

Сценарий метрика файлы атомлы яңынан яҙа, шулай итеп, һеҙҙең коллекционер һәр ваҡыт уҡый а
тулы файҙалы йөк.

Еңел иҫкәрткән өсөн, зонд сым һеҙҙең мониторинг стека. Prometheus
ике рәттән уңышһыҙлыҡтан һуң биттәрҙе миҫал:

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

### Rollback автоматлаштырыу

Идара итеү ярҙамсыһы ҡулланыу өсөн яңыртыу йәки тергеҙергә маҡсатлы Torii URL. Сценарий
һаҡлана, элекке конфигурация `.env.tryit-proxy.bak` шулай итеп, recongrobbleks был
бер команда.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

`--env` йәки `TRYIT_PROXY_ENV` менән env файл юлына өҫтөнлөк бирегеҙ, әгәр һеҙҙең таратыу
башҡа урындарҙа конфигурация һаҡлай.