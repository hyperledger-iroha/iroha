---
lang: kk
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Деректердің қолжетімділігі қауіп үлгісін автоматтандыру (DA-1)

Жол картасының DA-1 және `status.md` тармақтары детерминирленген автоматтандыру циклін шақырады
Norito PDP/PoTR қауіп үлгісінің қорытындыларын шығарады.
`docs/source/da/threat_model.md` және Docusaurus айнасы. Бұл каталог
сілтеме жасаған артефактілерді түсіреді:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (ол `scripts/docs/render_da_threat_model_tables.py` жұмыс істейді)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Ағын

1. **Есепті жасау**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON жиыны симуляцияланған репликацияның сәтсіз жылдамдығын, чұнкерді жазады
   шекті мәндер және PDP/PoTR жүйесі арқылы анықталған кез келген саясат бұзушылықтар
   `integration_tests/src/da/pdp_potr.rs`.
2. **Markdown кестелерін көрсетіңіз**
   ```bash
   make docs-da-threat-model
   ```
   Бұл қайта жазу үшін `scripts/docs/render_da_threat_model_tables.py` іске қосады
   `docs/source/da/threat_model.md` және `docs/portal/docs/da/threat-model.md`.
3. JSON есебін (және қосымша CLI журналын) көшіру арқылы **артефактты мұрағаттау**
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Қашан
   басқару шешімдері белгілі бір іске сүйенеді, git commit хэшін және қамтиды
   `<timestamp>-metadata.md` бауырындағы симулятор тұқымы.

## Дәлелді күту

- JSON файлдары git ішінде тұруы үшін <100 КБ қалуы керек. Үлкенірек орындау
  іздер сыртқы жадқа жатады — метадеректердегі олардың қол қойылған хэшіне сілтеме
  қажет болса ескертіңіз.
- Әрбір мұрағатталған файл тұқымды, конфигурация жолын және симулятор нұсқасын осылай көрсетуі керек
  DA шығару қақпаларын тексеру кезінде қайталауларды дәл ойнатуға болады.
- Кез келген уақытта `status.md` мұрағатталған файлға немесе жол картасы жазбасына сілтеме жасаңыз
  DA-1 қабылдау критерийлерін алға жылжытып, рецензенттер тексере алатынын қамтамасыз етеді
  әбзелді қайта іске қоспай-ақ базалық.

## Міндеттемені салыстыру (секвенджерді жіберіп алу)

DA қабылдау түбіртектерін салыстыру үшін `cargo xtask da-commitment-reconcile` пайдаланыңыз
DA міндеттеме жазбалары, секвенсерді жіберіп алу немесе бұрмалау:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito немесе JSON пішініндегі түбіртектерді және міндеттемелерді қабылдайды
  `SignedBlockWire`, `.norito` немесе JSON бумалары.
- Блок журналында кез келген билет жоқ болғанда немесе хэштер бөлінген кезде сәтсіздікке ұшырайды;
  `--allow-unexpected` әдейі аумақты қамтыған кезде тек блоктау билеттерін елемейді.
  түбіртек жинағы.
- Шығарылған JSON файлын жіберіп алу үшін басқару пакеттеріне/Alertmanager-ге тіркеңіз
  ескертулер; әдепкі бойынша `artifacts/da/commitment_reconciliation.json`.

## Артықшылық аудиті (тоқсан сайынғы қолжетімділікті шолу)

DA манифест/қайталау каталогтарын сканерлеу үшін `cargo xtask da-privilege-audit` пайдаланыңыз
(плюс қосымша қосымша жолдар) жоқ, каталог емес немесе әлемде жазылуы мүмкін
жазбалар:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Берілген Torii конфигурациясынан DA қабылдау жолдарын оқиды және Unix-ті тексереді
  рұқсаттар бар жерде.
- Жоқ/каталог емес/әлемде жазылмайтын жолдарды белгілейді және нөлдік емес шығуды қайтарады
  мәселелер туындаған кезде код.
- JSON жинағына қол қойып, тіркеңіз (`artifacts/da/privilege_audit.json` by
  әдепкі) пакеттерге және бақылау тақталарына тоқсан сайынғы қатынасу-қарау.