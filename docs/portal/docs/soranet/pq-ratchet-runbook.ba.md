---
lang: ba
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 370733f000ffa7022cab18931c2697031a225d2ce3ae83382896c0d61f9fe6e2
source_last_modified: "2026-01-05T09:28:11.912569+00:00"
translation_last_reviewed: 2026-02-07
id: pq-ratchet-runbook
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

## Ниәт

Был runbook ут-быраулы эҙмә-эҙлеклелек өсөн етәкселек итә SoraNet&#8217;s сәхнәләштерелгән пост-квант (PQ) анонимлыҡ сәйәсәте. Операторҙар репетиция ике промоушен (Стаж А -> Сәхнәлә В -> Этап) һәм контролдә тотолған девант кире I этапҡа B/A ҡасан PQ тәьмин итеү төшә. Бурау телеметрия ҡармаҡтарын раҫлай (I18NI0000014X, I18NI0000015X, `sorafs_orchestrator_pq_ratio_*`) һәм инцидент репетиция журналы өсөн артефакттар йыя.

## Алдан шарттар

- Һуңғы I18NI000000017X бинар мөмкинлектәрен-ауыртыу менән (I18NI0000000018X-та күрһәтелгән быраулау һылтанмаһында йәки унан һуң йөкләмәләр).
- I18NT00000000000000000002X стека I18NI0000000019X өйөмөнә инеү.
- Номиналь һаҡсы каталогы снимок. Бурау алдынан күсермәһен алыу һәм раҫлау:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Әгәр ҙә сығанаҡ каталогы JSON ғына баҫтырһа, уны I18NI000000020X менән I18NT000000004X бинарына яңынан кодлау ротация ярҙамсыларын эшләткәнсе.

- CLI менән метамағлүмәттәрҙе һәм этапҡа тиклемге эмитентын әйләнеш артефакттарын тотоу:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Селтәрҙәр һәм күҙәтеүсәнлек ярҙамында раҫланған тәҙрәне шылтыратыу командалары.

## Промоушен аҙымдары

1. **Стаж аудит**

   Башланғыс этапты яҙып алыу:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Көтөү I18NI000000021X промоушен алдынан.

2. **В этапҡа промотация (Күпселек PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Көтөп >=5 минут өсөн манифест яңыртыу өсөн.
   - I18NT000000003X (I18NI000000022X приборҙар таҡтаһы) раҫлай "Сәйәсәт ваҡиғалары" панелендә I18NI000000023X `stage=anon-majority-pq` өсөн күрһәтә.
   - скриншот йәки панель JSON тотоп, уны ваҡиға журналына беркетергә.

3. **С этапҡа пропагандалау (Ҡәтлы PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ``` X

   - I18NI000000025X гистограммалар тенденцияһы 1,0-ға тиклем.
   - Браунут прилавокты раҫлау тигеҙ ҡала; юғиһә һүтеү аҙымдарын үтәргә.

## Димоция / браунут бура

1. **Синтетик PQ етешмәүен индуцировать**

   Пусамент мөхитендә PQ эстафеталарын өҙөү, һаҡсыл каталогты классик яҙмаларға ғына ҡырҡып, оркестрлы кэшты яңынан тейәп ҡуйығыҙ:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Браунут телеметрияһын күҙәтегеҙ**

   - Приборҙар таҡтаһы: панель "Ҡырҡыу ставкаһы" 0-дан юғарыраҡ шырпы.
   - ПромQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` I18NI00000000029X менән `anonymity_outcome="brownout"` тураһында хәбәр итергә тейеш.

3. **В этапҡа тапшырыу / А** этап

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Әгәр PQ менән тәьмин итеү етерлек булмаһа, I18NI0000000300X-ҡа тиклем түбәнәйтегеҙ. Бурау тамамлай бер тапҡыр браунут иҫәпләүселәр урынлаша һәм акцияларҙы яңынан ҡулланырға мөмкин.

4. **Һаҡсы каталогты тергеҙергә**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметрия һәм артефакттар

- ** Приборҙар таҡтаһы:** `dashboards/grafana/soranet_pq_ratchet.json`
- **I18NT000000001X иҫкәртмәләр:** тәьмин итеү I18NI0000000032X браунут иҫкәртмә түбән ҡала конфигурацияланған SLO (<5% теләһә ниндәй 10 минут тәҙрә аша).
- **Инцидент лог:** әсирлеккә алынған телеметрия өҙөктәрен һәм оператор яҙмаларын I18NI000000033X-ҡа ҡуша.
- *Ҡулғалған тотоу:** ҡулланыу I18NI0000000034X бурау журналы һәм табло күсермәһен өсөн I18NI00000000035X, иҫәпләү BLAKE3 дигести, һәм етештереү өсөн ҡул ҡуйылған I18NI00000000006X.

Миҫал:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Генерацияланған метамағлүмәттәр һәм ҡултамға менән идара итеү пакетына беркетегеҙ.

## Rollback

Әгәр ҙә бурау ысын PQ етешмәүен аса, А этапта ҡала, селтәрле TL-ға хәбәр итә, йыйылған метрикаларҙы плюс һаҡсы каталогы инцидент трекерына беркетегеҙ. Ҡулланыу һаҡсы каталог экспорты алдан төшөрөлгән, ғәҙәти хеҙмәт тергеҙеү өсөн.

:::бик Регрессия ҡаплауы
I18NI000000037X синтетик валидация тәьмин итә, был дрель ярҙамында.
::: 1990 й.