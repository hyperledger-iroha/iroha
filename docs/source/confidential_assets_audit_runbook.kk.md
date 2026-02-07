---
lang: kk
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M4` сілтемесі бойынша құпия активтер аудиті және операциялар кітабы.

# Құпия активтер аудиті және операциялардың жұмыс кітабы

Бұл нұсқаулық аудиторлар мен операторлар сүйенетін дәлелдемелерді біріктіреді
құпия активтер ағындарын тексеру кезінде. Ол айналмалы ойын кітабын толықтырады
(`docs/source/confidential_assets_rotation.md`) және калибрлеу кітабы
(`docs/source/confidential_assets_calibration.md`).

## 1. Таңдамалы ашып көрсету және оқиға арналары

- Әрбір құпия нұсқау құрылымдық `ConfidentialEvent` пайдалы жүктемесін шығарады
  (`Shielded`, `Transferred`, `Unshielded`) түсірілген
  `crates/iroha_data_model/src/events/data/events.rs:198` және серияланған
  орындаушылар (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  Регрессия жинағы нақты пайдалы жүктемелерді орындайды, осылайша аудиторлар сене алады
  детерминирленген JSON орналасулары (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii бұл оқиғаларды стандартты SSE/WebSocket құбыры арқылы көрсетеді; аудиторлар
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) арқылы жазылу,
  таңдау бойынша бір актив анықтамасына ауқымды бөлу. CLI мысалы:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Саясат метадеректері және күтудегі ауысулар арқылы қол жетімді
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), Swift SDK арқылы бейнеленген
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) және құжатталған
  құпия активтер дизайны және SDK нұсқаулықтары
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Телеметрия, бақылау тақталары және калибрлеу дәлелдері

- Орындалу уақыты метрикасы ағаш тереңдігі, міндеттеме/шекара тарихы, түбірді шығару
  есептегіштер және тексеруші-кэшті соққы коэффициенттері
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana бақылау тақталары
  `dashboards/grafana/confidential_assets.json` байланысты панельдерді және
  `docs/source/confidential_assets.md:401` құжатталған жұмыс процесі бар ескертулер.
- Қол қойылған журналдары бар калибрлеу жұмыстары (NS/op, gas/op, ns/gas)
  `docs/source/confidential_assets_calibration.md`. Ең соңғы Apple Silicon
  NEON жүгірісі мұрағатталған
  `docs/source/confidential_assets_calibration_neon_20260428.log` және бірдей
  Бухгалтерлік кітап SIMD-бейтарап және AVX2 профильдері үшін уақытша бас тартуды жазады
  x86 хосттары желіге кіреді.

## 3. Оқиғаға әрекет ету және оператордың тапсырмалары

- Айналдыру/жаңарту процедуралары орналасқан
  `docs/source/confidential_assets_rotation.md`, жаңаны қалай сахналау керектігін қамтиды
  параметр жинақтары, саясат жаңартуларын жоспарлау және әмияндарды/аудиторларды хабардар ету. The
  трекер (`docs/source/project_tracker/confidential_assets_phase_c.md`) тізімдері
  runbook иелері және репетиция күтулері.
- Өндірістік жаттығулар немесе апаттық терезелер үшін операторлар дәлелдемелерді қосады
  `status.md` жазбалары (мысалы, көп жолақты репетиция журналы) және мыналарды қамтиды:
  `curl` саясат ауысуының дәлелі, Grafana суреті және сәйкес оқиға
  дайджест жасайды, осылайша аудиторлар жалбыз → тасымалдау → уақыт кестесін ашады.

## 4. Сыртқы шолу ырғағы

- Қауіпсіздікті шолу ауқымы: құпия схемалар, параметрлер тізілімдері, саясат
  ауысулар және телеметрия. Бұл құжат пен калибрлеу журналының пішіндері
  жеткізушілерге жіберілген дәлелдеме пакеті; шолу кестесі арқылы бақыланады
  `docs/source/project_tracker/confidential_assets_phase_c.md` ішіндегі M4.
- Операторлар `status.md` кез келген жеткізушінің қорытындыларымен немесе бақылаумен жаңартылып отыруы керек
  әрекет элементтері. Сыртқы шолу аяқталмайынша, бұл жұмыс кітабы ретінде қызмет етеді
  операциялық базалық аудиторлар сынай алады.