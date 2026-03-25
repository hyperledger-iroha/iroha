---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Резерв+жалға саясаты (жол картасының тармағы **SFM‑6**) енді `sorafs reserve` жібереді
CLI көмекшілері плюс `scripts/telemetry/reserve_ledger_digest.py` аудармашысы
қазынашылық операциялар детерминирленген жалдау/резервтік аударымдарды шығаруы мүмкін. Бұл бет айналады
жұмыс процесі `docs/source/sorafs_reserve_rent_plan.md` ішінде анықталған және түсіндіреді
жаңа трансфер арнасын Grafana + Alertmanager жүйесіне қалай қосуға болады, сондықтан экономика және
басқаруды тексерушілер әрбір есеп айырысу циклін тексере алады.

## Жұмыс процесі

1. **Дәйексөз + кітап проекциясы**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Бухгалтерлік кітап көмекшісі `ledger_projection` блогын қосады (жалдау мерзімі, резерв
   тапшылық, толықтыру дельтасы, андеррайтинг логикалық мәндер) плюс Norito `Transfer`
   ISI XOR-ды қазынашылық және резервтік шоттардың арасында жылжыту үшін қажет.

2. **Дайджест + Prometheus/NDJSON шығыстарын жасаңыз**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Дайджест көмекшісі micro‑XOR жиынтықтарын XOR-ға қалыпқа келтіреді, бар-жоғын жазады
   проекция андеррайтингке сәйкес келеді және **тасымалдау арнасы** көрсеткіштерін шығарады
   `sorafs_reserve_ledger_transfer_xor` және
   `sorafs_reserve_ledger_instruction_total`. Бірнеше кітап болуы керек кезде
   өңделген (мысалы, провайдерлер партиясы), `--ledger`/`--label` жұптарын қайталаңыз және
   көмекші әрбір дайджесті бар жалғыз NDJSON/Prometheus файлын жазады.
   бақылау тақталары бүкіл циклды тапсырыс бойынша желімсіз қабылдайды. `--out-prom`
   файл түйінді экспорттаушы мәтіндік файл жинағышына бағытталған — `.prom` файлын
   экспорттаушының қаралған каталогын немесе оны телеметрия шелегіне жүктеңіз
   `--ndjson-out` бірдей берілгенде, Reserve бақылау тақтасының жұмысы тұтынылады
   деректер құбырларына пайдалы жүктемелер.

3. **Артефактілерді + дәлелдемелерді жариялау**
   - Дайджесттерді `artifacts/sorafs_reserve/ledger/<provider>/` астында сақтаңыз және сілтеме жасаңыз
     апта сайынғы экономика есебіндегі Markdown қысқаша мазмұны.
   - JSON дайджестін жалға алудың күйіп кетуіне тіркеңіз (аудиторлар
     математика) және бақылау сомасын басқару дәлелдемелері пакетіне қосыңыз.
   - Егер дайджест толтыру немесе андеррайтингті бұзу туралы сигнал берсе, ескертуге сілтеме жасаңыз
     Идентификаторлар (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) және қандай ISI тасымалдау екенін ескеріңіз
     қолданылған.

## Көрсеткіштер → бақылау тақталары → ескертулер

| Бастапқы көрсеткіш | Grafana панелі | Ескерту / саясат ілгегі | Ескертпелер |
|-------------|---------------|---------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` ішіндегі “DA Rent Distribution (XOR/сағ)” | Апталық қазына дайджестін тамақтандырыңыз; резервтік ағындағы өсу `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) дейін таралады. |
| `torii_da_rent_gib_months_total` | «Сыйымдылықты пайдалану (ГиБ-айлар)» (сол бақылау тақтасы) | Шот-фактуралық сақтаудың XOR аударымдарына сәйкес келетінін дәлелдеу үшін кітап дайджестімен жұптаңыз. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | `dashboards/grafana/sorafs_reserve_economics.json` ішіндегі “Reserve Snapshot (XOR)” + күй карталары | `SoraFSReserveLedgerTopUpRequired` `requires_top_up=1` кезде жанады; `SoraFSReserveLedgerUnderwritingBreach` `meets_underwriting=0` кезінде жанады. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | `dashboards/grafana/sorafs_reserve_economics.json` ішіндегі «Түрі бойынша аударымдар», «Соңғы аударымдарды бөлу» және қамту карталары | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` және `SoraFSReserveLedgerTopUpTransferMissing` жалға беру/толықтыру қажет болса да, тасымалдау арнасы жоқ немесе нөлге тең болғанда ескертеді; қамту карталары бірдей жағдайларда 0%-ға дейін төмендейді. |

Жалға алу циклі аяқталғанда, Prometheus/NDJSON суреттерін жаңартыңыз, растаңыз
Grafana панельдері жаңа `label` таңдап, скриншоттарды қосады +
Жалға беруді басқару пакетіне ескертуші идентификаторлары. Бұл CLI проекциясын дәлелдейді,
телеметрия және басқару артефактілерінің барлығы **бір** тасымалдау арнасынан және
жол картасының экономика бақылау тақталарын резервтік+жалға алумен сәйкестендіреді
автоматтандыру. Қамту карталары 100% (немесе 1.0) және жаңа ескертулерді оқуы керек
дайджестте жалдау және резервтік толықтыру аударымдары болған кезде тазартылуы керек.