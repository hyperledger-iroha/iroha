---
lang: ba
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

# Мәғлүмәттәр-мөмкинлек хәүеф-модель автоматлаштырыу (DA-1)

Юл картаһы пункты DA-1 һәм I18NI0000000009X детерминистик автоматлаштырыу иллюминаторы өсөн саҡырыу, тип
етештереү I18NT00000000001X PDP/PoTR хәүеф-модель резюмелары 2019 йылда сыға.
`docs/source/da/threat_model.md` һәм I18NT000000000X көҙгөһө. Был каталог
һылтанма яһалған артефакттарҙы тота:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (ул `scripts/docs/render_da_threat_model_tables.py` эшләй)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Ағым

1. **Отчетты генерациялау**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON йомғаҡлау репликация етешһеҙлектәре тиҙлеген моделләштереү, chunker .
   сиктәре, һәм теләһә ниндәй сәйәсәт боҙоуҙар асыҡланған PDP/PoTR йүгән 2019 йылда .
   `integration_tests/src/da/pdp_potr.rs`.
2. **Маркдаун өҫтәлдәрен Рендер**
   ```bash
   make docs-da-threat-model
   ```
   Был I18NI000000018X эшләй, яңынан яҙырға
   `docs/source/da/threat_model.md` һәм I18NI000000020X.
3. **архив артефакт** JSON отчетын күсереп (һәм өҫтәмә CLI журналы)
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Ҡасан
   идара итеү ҡарарҙары аныҡ йүгерәгә таяна, git dor of Hish һәм
   симулятор орлоғо бер туған I18NI000000022X.

## Дәлилдәр көтөү

- JSON файлдар ҡалырға тейеш <100 KiB, шулай итеп, улар git йәшәй ала. Ҙурыраҡ башҡарыу
  эҙҙәр тышҡы һаҡлауға ҡарай — уларҙы метамағлүмәттәрҙә ҡул ҡуйылған хеш һылтанма
  кәрәк булһа, иғтибар итегеҙ.
- Һәр архивлы файл орлоҡ, конфигурациялау юлын һәм тренажер версияһын исемлеккә индерергә тейеш.
  перерасписаниеһы тап ҡасан аудит DA ҡапҡаларын сығарыуҙы ҡабатларға мөмкин.
- I18NI000000023X йәки юл картаһы яҙмаһы архив файлына кире һылтанма ҡасан ҡасан.
  DA-1 ҡабул итеү критерийҙары алға, тәьмин итеү рецензенттар раҫлай ала
  жгутты ҡабаттан эшләтмәйенсә башланғыс һыҙығы.

## йөкләмәһен яраштырыу (Эҙмә-эҙлеклелек ҡабул итеү)

Ҡулланыу I18NI000000024X сағыштырыу өсөн DA gingest квитанциялар ҡаршы .
DA йөкләмә яҙмалар, тотоу секвенсор йәки үҙгәртеп ҡороу:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- I18NT000000002X йәки JSON формаһы һәм йөкләмәләрендә ҡабул итеү квитанциялары
  `SignedBlockWire`, `.norito`, йәки JSON өйөмдәре.
- Блок журналынан йәки айырылышҡанда ниндәй ҙә булһа билет юғалғанда уңышһыҙлыҡтар;
  I18NI000000027X блок-тик билеттарҙы иғтибарға алмай, ҡасан һеҙ аңлы рәүештә масштаблы
  квитанция комплекты.
- JSON-ды идара итеү пакеттарына/Alertmanager-ға беркетергә кәрәк
  иҫкәртмәләр; ғәҙәттәгесә `artifacts/da/commitment_reconciliation.json` тиклем.

## өҫтөнлөклө аудит (кварталға инеү тикшерелгән)

Ҡулланыу I18NI000000029X сканерлау өсөн DA манифест/реплей каталогтары .
(плюс өҫтәмә юлдар) өсөн юҡ, каталогһыҙ, йәки донъяға яҙылған
яҙмалар:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- DA нәфрәтле юлдарҙы уҡый, бирелгән I18NT0000000003X конфигы һәм тикшерергә Unix .
  рөхсәттәр ҡайҙа бар.
- Флагтар юҡ/а-каталог түгел/донъя яҙыулы юлдар һәм нулдән тыш сығыу юлын ҡайтара
  код ҡасан мәсьәләләр бар.
- Билдәһе һәм JSON өйөмөн беркетергә (I18NI0000000030X тиклем.
  ғәҙәттәгесә) квартал һайын инеү-тикшерелгән пакеттар һәм приборҙар таҡтаһы.