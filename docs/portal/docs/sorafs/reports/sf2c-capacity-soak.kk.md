---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-12-29T18:16:35.201180+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SF-2c сыйымдылығының есептелуі туралы есеп

Күні: 21.03.2026 ж

## Ауқым

Бұл есепте детерминирленген SoraFS сыйымдылық есептелуі мен төлемді сіңіру жазылады.
SF-2c жол картасы жолында сұралған сынақтар.

- **30-күндік мультипровайдер сору:** Жаттығуды орындаған
  `capacity_fee_ledger_30_day_soak_deterministic` дюйм
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Жабдық бес провайдерді жасайды, 30 есеп айырысу терезесін қамтиды және
  Бухгалтерлік кітап қорытындылары тәуелсіз есептелген анықтамаға сәйкес келетінін растайды
  проекция. Сынақ Blake3 дайджестін (`capacity_soak_digest=...`) шығарады, сондықтан
  CI канондық суретті түсіріп, ажырата алады.
- **Жеткізу үшін айыппұлдар:** Орындаған
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (бірдей файл). Сынақ ереуіл шегін, салқындатуды, кепілдік қиғаш сызықтарды,
  және бухгалтерлік есептегіштер детерминистік болып қалады.

## Орындау

Жібітуді тексеруді жергілікті түрде іске қосыңыз:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Сынақтар стандартты ноутбукта бір секундтан аз уақыт ішінде аяқталады және жоқ
сыртқы қондырғылар.

## Бақылау мүмкіндігі

Torii енді бақылау тақталары үшін төлем журналдарымен бірге провайдердің несиелік суреттерін көрсетеді
төмен теңгерім мен айыппұл соққыларына қарсы тұра алады:

- REST: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` жазбаларын қайтарады
  жібіту сынағында тексерілген кітап өрістерін айналаңыз. Қараңыз
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana импорт: `dashboards/grafana/sorafs_capacity_penalties.json`
  экспортталған strike counters, өсімпұл сомасы және кепілге қойылған кепілді талап бойынша
  қызметкерлер негізгі көрсеткіштерді тірі ортамен салыстыра алады.

## Бақылау

- Ылғалдау сынамасын (түтін деңгейін) қайталау үшін CI-де апта сайынғы жұмыстарды жоспарлаңыз.
- Өндірістік телеметриядан кейін Grafana тақтасын Torii қырып алу мақсаттарымен кеңейтіңіз
  экспорт өмірге келеді.