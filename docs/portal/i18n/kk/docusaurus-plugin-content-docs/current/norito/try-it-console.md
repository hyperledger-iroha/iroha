---
lang: kk
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Портал Torii трафикті жіберетін үш интерактивті бетті жинақтайды:

- `/reference/torii-swagger` мекенжайындағы **Swagger UI** қол қойылған OpenAPI спецификациясын көрсетеді және `TRYIT_PROXY_PUBLIC_URL` орнатылған кезде прокси арқылы сұрауларды автоматты түрде қайта жазады.
- `/reference/torii-rapidoc` мекенжайындағы **RapiDoc** файлды жүктеп салулармен және `application/x-norito` үшін жақсы жұмыс істейтін мазмұн түрі селекторларымен бірдей схеманы көрсетеді.
- **Оны қолданып көріңіз құмсалғыш** Norito шолу бетіндегі арнайы REST сұраулары мен OAuth құрылғысына кіру үшін жеңіл пішінді қамтамасыз етеді.

Барлық үш виджет сұрауларды жергілікті **Try-It проксиге** (`docs/portal/scripts/tryit-proxy.mjs`) жібереді. Прокси `static/openapi/torii.json` `static/openapi/manifest.json` жүйесіндегі қол қойылған дайджестке сәйкес келетінін тексереді, жылдамдықты шектегішті мәжбүрлейді, журналдардағы `X-TryIt-Auth` тақырыптарын өңдейді және `X-TryIt-Client` арқылы әрбір жоғары ағындық қоңырауды белгілейді, осылайша I010 трафик көзін тексере алады.

## Проксиді іске қосыңыз

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` - сіз орындағыңыз келетін Torii негізгі URL мекенжайы.
- `TRYIT_PROXY_ALLOWED_ORIGINS` консоль ендіруі керек әрбір порталдың шығу тегі (жергілікті әзірлеу сервері, өндірістік хост атауы, алдын ала қарау URL мекенжайы) қамтуы керек.
- `TRYIT_PROXY_PUBLIC_URL` `docusaurus.config.js` арқылы тұтынылады және `customFields.tryIt` арқылы виджеттерге енгізіледі.
- `TRYIT_PROXY_BEARER` тек `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` кезде жүктейді; әйтпесе пайдаланушылар консоль немесе OAuth құрылғы ағыны арқылы өз таңбалауышын қамтамасыз етуі керек.
- `TRYIT_PROXY_CLIENT_ID` әрбір сұрауда тасымалданатын `X-TryIt-Client` тегін орнатады.
  Браузерден `X-TryIt-Client` жеткізуге рұқсат етілген, бірақ мәндер кесілген
  және оларда басқару таңбалары болса, қабылданбайды.

Іске қосу кезінде прокси `verifySpecDigest` іске қосады және манифест ескірген болса, түзету нұсқауымен шығады. Ең жаңа Torii спецификациясын жүктеп алу үшін `npm run sync-openapi -- --latest` іске қосыңыз немесе төтенше жағдайды қайта анықтау үшін `TRYIT_PROXY_ALLOW_STALE_SPEC=1` өтіңіз.

Орташа файлдарды қолмен өңдеусіз прокси мақсатты жаңарту немесе кері айналдыру үшін көмекшіні пайдаланыңыз:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Виджеттерді жалғаңыз

Прокси тыңдап болғаннан кейін порталға қызмет көрсету:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` келесі түймелерді көрсетеді:

| Айнымалы | Мақсаты |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL URL Swagger, RapiDoc және Try it құм жәшігіне енгізілген. Рұқсат етілмеген алдын ала қарау кезінде виджеттерді жасыру үшін орнатусыз қалдырыңыз. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Қосымша әдепкі таңбалауыш жадта сақталады. `DOCS_SECURITY_ALLOW_INSECURE=1` жергілікті түрде өтпесеңіз, `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` және HTTPS тек CSP қорғаушысы (DOCS-1b) қажет. |
| `DOCS_OAUTH_*` | OAuth құрылғы ағынын (`OAuthDeviceLogin` құрамдас) қосыңыз, осылайша шолушылар порталдан шықпай-ақ қысқа мерзімді таңбалауыштарды жасай алады. |

OAuth айнымалы мәндері болған кезде құмсалғыш конфигурацияланған аутентификация сервері арқылы өтетін **Құрылғы кодымен кіру** түймешігін көрсетеді (нақты пішінді `config/security-helpers.js` қараңыз). Құрылғы ағыны арқылы шығарылған таңбалауыштар тек шолғыш сеансында кэштеледі.

## Norito-RPC пайдалы жүктемелерін жіберу

1. [Norito жылдам бастау](./quickstart.md) бөлімінде сипатталған CLI немесе үзінділер арқылы `.norito` пайдалы жүктемесін жасаңыз. Прокси `application/x-norito` денелерін өзгеріссіз қайта жібереді, осылайша `curl` арқылы жариялаған бірдей артефактты қайта пайдалануға болады.
2. `/reference/torii-rapidoc` (екілік пайдалы жүктемелер үшін қолайлы) немесе `/reference/torii-swagger` ашыңыз.
3. Ашылмалы тізімнен қажетті Torii суретін таңдаңыз. Суреттерге қол қойылады; панель `static/openapi/manifest.json` ішінде жазылған манифест дайджестін көрсетеді.
4. “Байқап көру” тартпасында `application/x-norito` мазмұн түрін таңдап, **Файлды таңдау** түймесін басып, пайдалы жүктемені таңдаңыз. Прокси сұрауды `/proxy/v1/pipeline/submit` түріне қайта жазады және оны `X-TryIt-Client=docs-portal-rapidoc` белгісімен белгілейді.
5. Norito жауаптарын жүктеп алу үшін `Accept: application/x-norito` орнатыңыз. Swagger/RapiDoc бір жәшіктегі тақырып селекторын ашады және екілік файлды прокси арқылы кері ағынмен жібереді.

Тек JSON маршруттары үшін ендірілген "Тапсырма" құмсалғышы жиі жылдамырақ болады: жолды енгізіңіз (мысалы, `/v1/accounts/soraカタカナ.../assets`), HTTP әдісін таңдаңыз, қажет болғанда JSON негізгі мәтінін қойыңыз және тақырыптарды, ұзақтығын және кірістірілген пайдалы жүктемелерді тексеру үшін **Сұраныс жіберу** түймесін басыңыз.

## Ақаулықтарды жою

| Симптом | Ықтимал себебі | Түзету |
| --- | --- | --- |
| Браузер консолі CORS қателерін көрсетеді немесе құм жәшік прокси URL мекенжайының жоқ екенін ескертеді. | Прокси жұмыс істемейді немесе түпнұсқа ақ тізімде жоқ. | Проксиді іске қосыңыз, `TRYIT_PROXY_ALLOWED_ORIGINS` портал хостыңызды қамтитынына көз жеткізіңіз және `npm run start` қайта іске қосыңыз. |
| `npm run tryit-proxy` "дайджест сәйкессіздігімен" шығады. | Torii OpenAPI бумасы ағынға қарай өзгерді. | `npm run sync-openapi -- --latest` (немесе `--version=<tag>`) іске қосыңыз және әрекетті қайталаңыз. |
| Виджеттер `401` немесе `403` қайтарады. | Токен жоқ, мерзімі өтіп кеткен немесе аумақтар жеткіліксіз. | OAuth құрылғы ағынын пайдаланыңыз немесе жарамды тасымалдаушы таңбалауышын құм жәшігіне қойыңыз. Статикалық таңбалауыштар үшін `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` экспорттауыңыз керек. |
| `429 Too Many Requests` проксиден. | Бір IP жылдамдығы шегінен асып кетті. | Сенімді орталар немесе дроссельді тексеру сценарийлері үшін `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` көтеріңіз. Барлық мөлшерлеме-лимитін қабылдамау өсімі `tryit_proxy_rate_limited_total`. |

## Бақылау мүмкіндігі

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs` айналасындағы орауыш) `/healthz` шақырады, қалауы бойынша үлгі маршрутты орындайды және `probe_success` / I100700. I18NI00000000 үшін Prometheus мәтіндік файлдарын шығарады. Node_exporter бағдарламасымен біріктіру үшін `TRYIT_PROXY_PROBE_METRICS_FILE` конфигурациялаңыз.
- `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` параметрін санауыштарды (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) және кідіріс гистограммаларын көрсету үшін орнатыңыз. `dashboards/grafana/docs_portal.json` тақтасы DOCS-SORA SLO талаптарын орындау үшін осы көрсеткіштерді оқиды.
- Орындалу уақыты журналдары stdout сайтында тікелей эфирде. Әрбір жазба сұрау идентификаторын, жоғарғы ағын күйін, аутентификация көзін (`default`, `override` немесе `client`) және ұзақтығын қамтиды; құпиялар шығарылғанға дейін өңделеді.

`application/x-norito` пайдалы жүктемелері өзгеріссіз Torii жететінін растау қажет болса, Jest Suite (`npm test -- tryit-proxy`) іске қосыңыз немесе `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` астындағы арматураларды тексеріңіз. Регрессия сынақтары қысылған Norito екілік файлдарын, қол қойылған OpenAPI манифесттерін және проксиді төмендету жолдарын қамтиды, осылайша NRPC шығарылымдары тұрақты дәлел ізін сақтайды.