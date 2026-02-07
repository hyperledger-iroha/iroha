---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ffbb145e1e0aa9dc71bdb6896c4f8be69eb6226194c5c165905af1ac243cc9
source_last_modified: "2025-12-29T18:16:35.199832+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
---

# SoraFS Сыйымдылық нарығын тексеру тізімі

**Шолу терезесі:** 2026-03-18 → 2026-03-24  
**Бағдарлама иелері:** Сақтау тобы (`@storage-wg`), Басқару кеңесі (`@council`), Қазынашылық гильдия (`@treasury`)  
**Қолдану аясы:** SF-2c GA үшін қажет провайдердің борттық құбырлары, дауларды қарау ағындары және қазынашылық салыстыру процестері.

Сыртқы операторлар үшін нарықты іске қоспас бұрын төмендегі бақылау тізімін қарап шығу керек. Әрбір жол аудиторлар қайталай алатын детерминирленген дәлелдерге (сынақтар, құрылғылар немесе құжаттамалар) сілтеме жасайды.

## Қабылдауды бақылау парағы

### Провайдерді қосу

| тексеру | Валидация | Дәлелдер |
|-------|------------|----------|
| Реестр канондық сыйымдылық туралы мәлімдемелерді қабылдайды | Қолтаңбаны өңдеуді, метадеректерді түсіруді және түйін тізіліміне жіберуді тексеретін API қолданбасы арқылы `/v1/sorafs/capacity/declare` интеграциялық сынақ жаттығулары. | `crates/iroha_torii/src/routing.rs:7654` |
| Ақылды келісімшарт сәйкес келмейтін пайдалы жүктемелерден бас тартады | Бірлік сынағы провайдер идентификаторлары мен бекітілген GiB өрістерінің сақталмас бұрын қол қойылған мәлімдемеге сәйкестігін қамтамасыз етеді. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI канондық қосу артефактілерін шығарады | CLI құрылғысы детерминирленген Norito/JSON/Base64 шығыстарын жазады және операторлар мәлімдемелерді офлайн режимде өткізуі үшін айналымдарды тексереді. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Оператор нұсқаулығы қабылдау жұмыс үрдісін және басқару қоршауларын түсіреді | Құжаттама мәлімдеме схемасын, саясаттың әдепкі мәндерін және кеңеске арналған шолу қадамдарын санайды. | `../storage-capacity-marketplace.md` |

### Дауларды шешу

| тексеру | Валидация | Дәлелдер |
|-------|------------|----------|
| Дау жазбалары канондық пайдалы жүк дайджестімен сақталады Бірлік сынағы дауды тіркейді, сақталған пайдалы жүктемені декодтайды және кітап детерминизміне кепілдік беру үшін күту күйін бекітеді. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI дау генераторы канондық схемаға сәйкес келеді | CLI сынағы `CapacityDisputeV1` үшін Base64/Norito шығыстарын және JSON қорытындыларын қамтиды, бұл дәлелдер бумаларының хэшті анықтауды қамтамасыз етеді. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Қайталау сынағы дау/айыппұл детерминизмін дәлелдейді | Екі рет қайталанған дәлелдеудің сәтсіздігі телеметриясы бірдей бухгалтерлік кітапты, несиені және даудың суретін жасайды, осылайша қиғаш сызықтар әріптестер арасында детерминистік болады. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook құжаттарының күшеюі және күшін жою ағыны | Операциялар нұсқаулығы кеңес жұмыс үрдісін, дәлел талаптарын және кері қайтару процедураларын қамтиды. | `../dispute-revocation-runbook.md` |

### Қазынашылықты салыстыру

| тексеру | Валидация | Дәлелдер |
|-------|------------|----------|
| Бухгалтерлік кітапты есептеу 30 күндік проекцияға сәйкес келеді Soak сынағы бес провайдерді 30 есеп айырысу терезесін қамтиды, бұл бухгалтерлік жазбаларды күтілетін төлем анықтамасына қарсы ерекшелендіреді. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Түнде тіркелетін бухгалтерлік экспортты салыстыру | `capacity_reconcile.py` төлем журналының күтулерін орындалған XOR аударымының экспорттарымен салыстырады, Prometheus көрсеткіштерін шығарады және Alertmanager арқылы қазынашылық мақұлдауын береді. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Төлем бақылау тақталары айыппұлдарды және есептеу телеметриясын | Grafana импорттық графиктер ГиБ·сағаттық есептеу, стрелка есептегіштері және шақыру кезінде көріну үшін бекітілген кепіл. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Жарияланған есеп мұрағаттары сіңу әдістемесі мен пәрмендерді қайталау | Есептің егжей-тегжейлері аудиторларға арналған ауқымды, орындау пәрмендерін және бақылау ілмектерін сіңіреді. | `./sf2c-capacity-soak.md` |

## Орындау туралы ескертпелер

Тіркеуден шығу алдында тексеру жинағын қайта іске қосыңыз:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторлар `sorafs_manifest_stub capacity {declaration,dispute}` көмегімен қосу/дауласу сұрауының пайдалы жүктемелерін қайта жасауы және нәтиже JSON/Norito байттарын басқару билетімен бірге мұрағаттауы керек.

## Тіркеу артефактілері

| Артефакт | Жол | blake2b-256 |
|----------|------|-------------|
| Провайдердің қосылуын растау пакеті | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Дауларды шешуді бекіту пакеті | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Қазынашылық салыстыруды мақұлдау пакеті | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Осы артефактілердің қол қойылған көшірмелерін шығару бумасымен бірге сақтаңыз және оларды басқаруды өзгерту жазбасында байланыстырыңыз.

## Бекітулер

- Сақтау тобының жетекшісі — @storage-tl (24.03.2026)  
- Басқару кеңесінің хатшысы — @council-sec (2026-03-24)  
- Қазынашылық операциялар бойынша жетекші — @treasury-ops (2026-03-24)