---
lang: kk
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fcd909a7013c5147e4f0c89c67de856ff56797b99281b954c7708ad83ab5cdc8
source_last_modified: "2026-01-28T17:11:30.699790+00:00"
translation_last_reviewed: 2026-02-07
id: address-checksum-runbook
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for I105 checksum failures (ADDR-7).
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
Бұл бет `docs/source/sns/address_checksum_failure_runbook.md` көрсетеді. Жаңарту
алдымен бастапқы файлды, содан кейін осы көшірмені синхрондаңыз.
:::

Бақылау сомасының қателері `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) болып шығады
Torii, SDK және әмиян/барлаушы клиенттері. ADDR-6/ADDR-7 жол картасының элементтері қазір
операторлардан бақылау сомасы ескертулері немесе қолдау көрсету кезінде осы жұмыс кітабын орындауды талап етеді
билеттер өртенді.

## Ойынды қашан іске қосу керек

- **Ескертулер:** `AddressInvalidRatioSlo` (белгіленген
  `dashboards/alerts/address_ingest_rules.yml`) сапарлар және аннотациялар тізімі
  `reason="ERR_CHECKSUM_MISMATCH"`.
- **Фиктураның дрейфі:** `account_address_fixture_status` Prometheus мәтіндік файлы немесе
  Grafana бақылау тақтасы кез келген SDK көшірмесі үшін бақылау сомасының сәйкессіздігі туралы хабарлайды.
- **Қолдау күшейту:** Әмиян/зерттеуші/SDK командалары бақылау сомасы қателеріне сілтеме жасайды, IME
  сыбайлас жемқорлық немесе бұдан былай декодталмаған алмасу буферін сканерлеу.
- **Қолмен бақылау:** Torii журналдары қайталанатын `address_parse_error=checksum_mismatch` көрсетеді
  өндірістің соңғы нүктелері үшін.

Оқиға Local-8/Local-12 соқтығыстарына қатысты болса, мынаны орындаңыз
Оның орнына `AddressLocal8Resurgence` немесе `AddressLocal12Collision` ойын кітаптары.

## Дәлелдерді тексеру парағы

| Дәлелдер | Пәрмен / Орын | Ескертпелер |
|----------|-------------------|-------|
| Grafana суреті | `dashboards/grafana/address_ingest.json` | Жарамсыз себептердің бұзылуын және әсер еткен соңғы нүктелерді түсіріңіз. |
| Ескерту жүктемесі | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Мәтінмәндік белгілер мен уақыт белгілерін қосыңыз. |
| Фикстура денсаулық | `artifacts/account_fixture/address_fixture.prom` + Grafana | SDK көшірмелерінің `fixtures/account/address_vectors.json` нұсқасынан ауытқығанын дәлелдейді. |
| PromQL сұрауы | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Оқиға құжаты үшін CSV экспорттау. |
| Журналдар | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (немесе журналды біріктіру) | Бөлісу алдында PII сүртіңіз. |
| Бекітуді тексеру | `cargo xtask address-vectors --verify` | Канондық генераторды растайды және бекітілген JSON келіседі. |
| SDK теңдігін тексеру | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Ескертулерде/билеттерде көрсетілген әрбір SDK үшін іске қосыңыз. |
| Алмасу буфері/IME ақыл-ойы | `iroha tools address inspect <literal>` | Жасырын таңбаларды немесе IME қайта жазуларын анықтайды; сілтеме `address_display_guidelines.md`. |

## Жедел жауап

1. Ескертуді растаңыз, оқиғадағы Grafana суреттерін + PromQL шығысын байланыстырыңыз
   жіп және жазба әсер етті Torii мәтінмәндері.
2. Мекенжайды талдауға түртетін манифест жарнамаларын / SDK шығарылымдарын тоқтатыңыз.
3. Бақылау тақтасының суреттерін және жасалған Prometheus мәтіндік файлының артефактілерін мына жерде сақтаңыз.
   оқиға қалтасы (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. `checksum_mismatch` пайдалы жүктемелерін көрсететін журнал үлгілерін алыңыз.
5. SDK иелеріне (`#sdk-parity`) үлгі пайдалы жүктемелерімен хабарлаңыз, осылайша олар сыналады.

## Түбірлік себептерді оқшаулау

### Бекіткіш немесе генератордың дрейфі

- `cargo xtask address-vectors --verify` қайта іске қосыңыз; сәтсіз болса, қалпына келтіріңіз.
- `ci/account_fixture_metrics.sh` (немесе жеке
  `scripts/account_fixture_helper.py check`) жиынтықты растау үшін әрбір SDK үшін
  құрылғылар канондық JSON сәйкес келеді.

### Клиенттік кодтаушылар / IME регрессиялары

- Ені нөлді табу үшін `iroha tools address inspect` арқылы пайдаланушы ұсынған литералдарды тексеріңіз
  қосылулар, кана түрлендірулер немесе қысқартылған пайдалы жүктемелер.
- Кросс-тексеру әмиян/барлаушы ағындарымен
  `docs/source/sns/address_display_guidelines.md` (қос көшірме нысандары, ескертулер,
  QR көмекшілері) бекітілген UX талаптарына сай келетініне көз жеткізу үшін.

### Манифест немесе тізілім мәселелері

- Соңғы манифест бумасын қайта тексеру үшін `address_manifest_ops.md` орындаңыз және
  ешбір Local-8 селекторының қайта өңделмегеніне көз жеткізіңіз.
  пайдалы жүктемелерде пайда болады.

### Зиянды немесе дұрыс емес трафик

- Torii журналдары және `torii_http_requests_total` арқылы бұзылған IP мекенжайларын/қолданба идентификаторларын бөліңіз.
- Қауіпсіздік/басқару бақылауы үшін журналдарды кемінде 24 сағат сақтаңыз.

## Салдарларды азайту және қалпына келтіру

| Сценарий | Әрекеттер |
|----------|---------|
| Арматураның дрейфі | `fixtures/account/address_vectors.json` қалпына келтіріңіз, `cargo xtask address-vectors --verify` қайта іске қосыңыз, SDK бумаларын жаңартыңыз және `address_fixture.prom` суретін билетке тіркеңіз. |
| SDK/клиент регрессиясы | Канондық қондырғыға + `iroha tools address inspect` шығысына және SDK паритеті CI (мысалы, `ci/check_address_normalize.sh`) артындағы қақпа шығарылымдарына сілтеме жасайтын файл мәселелері. |
| Зиянды жіберулер | Бағалауды шектеңіз немесе бұзатын директорларды блоктаңыз, егер құлпытас орнату таңдаушылары қажет болса, Басқаруға жеткізіңіз. |

Жеңілдетілген әрекеттер орындалғаннан кейін растау үшін жоғарыдағы PromQL сұрауын қайта іске қосыңыз
`ERR_CHECKSUM_MISMATCH` кем дегенде нөлде қалады (`/tests/*` қоспағанда)
Оқиғаны төмендетуге 30 минут қалғанда.

## Жабу

1. Grafana суретін, PromQL CSV, журнал үзінділерін және `address_fixture.prom` мұрағат.
2. Құралдар/құжаттар болса, `status.md` (ADDR бөлімі) және жол картасы жолын жаңартыңыз.
   өзгерді.
3. Жаңа сабақтар кезінде `docs/source/sns/incidents/` астындағы оқиғадан кейінгі жазбаларды файл жасаңыз
   пайда болады.
4. SDK шығарылымы ескертпелерінде мүмкін болған жағдайда бақылау сомасының түзетулері көрсетілгеніне көз жеткізіңіз.
5. Ескертудің 24 сағат бойы жасыл болып қалуын және арматураны тексеру алдында жасыл болып қала беретінін растаңыз
   шешу.