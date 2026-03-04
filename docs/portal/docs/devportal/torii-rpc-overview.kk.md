---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-12-29T18:16:35.115752+00:00"
translation_last_reviewed: 2026-02-07
title: Norito-RPC Overview
translator: machine-google-reviewed
---

# Norito-RPC шолу

Norito-RPC — Torii API интерфейстері үшін екілік тасымалдау. Ол бірдей HTTP жолдарын қайта пайдаланады
`/v1/pipeline` ретінде, бірақ схемасы бар Norito кадрлы пайдалы жүктемелерді ауыстырады
хэштер мен бақылау сомасы. Оны сізге детерминирленген, расталған жауаптар қажет болғанда пайдаланыңыз немесе
құбырдың JSON жауаптары тығырықтан шыққан кезде.

## Неліктен ауысу керек?
- CRC64 және схема хэштері бар детерминистік жақтау декодтау қателерін азайтады.
- SDK арқылы ортақ Norito көмекшілері бар деректер үлгісі түрлерін қайта пайдалануға мүмкіндік береді.
- Torii телеметрияда Norito сеанстарын тегтеп қойған, сондықтан операторлар бақылай алады.
берілген бақылау тақталарымен қабылдау.

## Өтініш жасау

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. Пайдалы жүктемені Norito кодекімен сериялаңыз (`iroha_client`, SDK көмекшілері немесе
   `norito::to_bytes`).
2. Сұрауды `Content-Type: application/x-norito` арқылы жіберіңіз.
3. `Accept: application/x-norito` арқылы Norito жауабын сұраңыз.
4. Сәйкес SDK көмекшісін пайдаланып жауапты декодтаңыз.

SDK-арнайы нұсқаулық:
- **Rust**: `iroha_client::Client` орнатылған кезде Norito автоматты түрде келіседі.
  `Accept` тақырыбы.
- **Python**: `iroha_python.norito_rpc` ішінен `NoritoRpcClient` пайдаланыңыз.
- **Android**: `NoritoRpcClient` және `NoritoRpcRequestOptions` пайдаланыңыз
  Android SDK.
- **JavaScript/Swift**: көмекшілер `docs/source/torii/norito_rpc_tracker.md` ішінде бақыланады
  және NRPC-3 бөлігі ретінде қонады.

## Оны пайдаланып көріңіз консоль үлгісі

Әзірлеуші порталы шолушылардың Norito қайта ойнатуы үшін, оны көріңіз проксиді жібереді.
тапсырыс сценарийлерін жазбай пайдалы жүктемелер.

1. [Проксиді іске қосыңыз](./try-it.md#start-the-proxy-locally) және орнатыңыз
   `TRYIT_PROXY_PUBLIC_URL`, сондықтан виджеттер трафикті қайда жіберу керектігін біледі.
2. Осы бетте **Байқап көру** картасын немесе `/reference/torii-swagger` ашыңыз.
   панелін таңдап, `POST /v1/pipeline/submit` сияқты соңғы нүктені таңдаңыз.
3. **Content-Type** параметрін `application/x-norito` күйіне ауыстырыңыз, **Екілік** параметрін таңдаңыз.
   өңдегішін орнатыңыз және `fixtures/norito_rpc/transfer_asset.norito` жүктеңіз
   (немесе тізімде көрсетілген кез келген пайдалы жүк
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. OAuth құрылғы коды виджеті немесе қолмен таңбалауыш арқылы жеткізуші таңбалауышын қамтамасыз етіңіз
   өріс (прокси арқылы конфигурацияланған кезде `X-TryIt-Auth` қайта анықтауды қабылдайды
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Сұрауды жіберіңіз және Torii тізімде көрсетілген `schema_hash` жаңғырығына сәйкес келетінін тексеріңіз.
   `fixtures/norito_rpc/schema_hashes.json`. Сәйкес келетін хэштер
   Norito тақырыбы браузерден/прокси-серверден аман қалды.

Жол картасының дәлелі үшін "Байқап көру" скриншотын іске қосумен жұптаңыз
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Сценарий аяқталады
`cargo xtask norito-rpc-verify`, JSON қорытындысын жазады
`artifacts/norito_rpc/<timestamp>/`, және сол арматураларды түсіреді
портал тұтынылды.

## Ақаулықтарды жою

| Симптом | Қай жерде пайда болады | Ықтимал себебі | Түзету |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii жауап | `Content-Type` тақырыбы жоқ немесе дұрыс емес | Пайдалы жүктемені жібермес бұрын `Content-Type: application/x-norito` орнатыңыз. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii жауап мәтіні/тақырыптары | Бекіту схемасының хэші Torii құрастырудан ерекшеленеді | Арматураларды `cargo xtask norito-rpc-fixtures` арқылы қалпына келтіріңіз және `fixtures/norito_rpc/schema_hashes.json` ішіндегі хэшті растаңыз; егер соңғы нүкте әлі Norito қосылмаған болса, JSON-ға оралыңыз. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Оны көріңіз прокси жауап | Сұрау `TRYIT_PROXY_ALLOWED_ORIGINS` | тізімінде жоқ түпнұсқадан келді Env var параметріне портал түпнұсқасын қосыңыз (мысалы, `https://docs.devnet.sora.example`) және проксиді қайта іске қосыңыз. |
| `{"error":"rate_limited"}` (HTTP 429) | Оны көріңіз прокси жауап | Бір IP квотасы `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` бюджетінен асты | Ішкі жүктемені тексеру үшін шектеуді арттырыңыз немесе терезе қалпына келтірілгенше күтіңіз (JSON жауапындағы `retryAfterMs` қараңыз). |
| `{"error":"upstream_timeout"}` (HTTP 504) немесе `{"error":"upstream_error"}` (HTTP 502) | Оны көріңіз прокси жауап | Torii күту уақыты бітті немесе прокси конфигурацияланған серверге жете алмады | `TRYIT_PROXY_TARGET` қол жетімді екенін тексеріңіз, Torii күйін тексеріңіз немесе үлкенірек `TRYIT_PROXY_TIMEOUT_MS` арқылы қайталап көріңіз. |

Қосымша "Байқап көру" диагностикасы және OAuth кеңестері бар
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Қосымша ресурстар
- Көлік RFC: `docs/source/torii/norito_rpc.md`
- Атқарушы қорытынды: `docs/source/torii/norito_rpc_brief.md`
- Әрекет трекері: `docs/source/torii/norito_rpc_tracker.md`
- Try-It прокси нұсқаулары: `docs/portal/docs/devportal/try-it.md`