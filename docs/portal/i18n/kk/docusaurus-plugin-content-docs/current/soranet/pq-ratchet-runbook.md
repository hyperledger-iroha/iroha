---
id: pq-ratchet-runbook
lang: kk
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ескерту Канондық дереккөз
:::

## Мақсат

Бұл жұмыс кітабы SoraNet-тің кезеңді пост-кванттық (PQ) анонимдік саясаты үшін өртке қарсы бұрғылау ретін бағыттайды. Операторлар жоғарылатуды (А кезеңі -> B кезеңі -> C кезеңі) және PQ жеткізуі төмендеген кезде, басқарылатын төмендетуді B/A кезеңіне қайтарады. Бұрғылау телеметрия ілгектерін (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) тексереді және оқиғаны қайталау журналы үшін артефактілерді жинайды.

## Алғышарттар

- Мүмкіндік салмағы бар соңғы `sorafs_orchestrator` екілік нұсқасы (`docs/source/soranet/reports/pq_ratchet_validation.md` ішінде көрсетілген бұрғылау сілтемесінде немесе одан кейін орындаңыз).
- `dashboards/grafana/soranet_pq_ratchet.json` қызмет көрсететін Prometheus/Grafana стекіне кіру.
- Номиналды қорғау каталогының суреті. Жаттығудан бұрын көшірмені алыңыз және тексеріңіз:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Егер бастапқы каталог тек JSON жарияласа, айналдыру көмекшілерін іске қоспас бұрын оны Norito екілік нұсқасына `soranet-directory build` арқылы қайта кодтаңыз.

- CLI көмегімен метадеректер мен кезең алдындағы эмитенттің айналу артефактілерін түсіріңіз:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Желілік және бақылауды шақыру бойынша топтар бекіткен өзгерту терезесі.

## Көтермелеу қадамдары

1. **Кезеңдік аудит**

   Бастапқы кезеңді жазыңыз:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Науқанға дейін `anon-guard-pq` күтіңіз.

2. **В кезеңіне көтерілу (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Манифесттерді жаңарту үшін >=5 минут күтіңіз.
   - Grafana (`SoraNet PQ Ratchet Drill` бақылау тақтасында) "Саясат оқиғалары" панелінде `stage=anon-majority-pq` үшін `outcome=met` көрсетіледі.
   - Скриншотты немесе JSON панелін түсіріп, оны оқиғалар журналына тіркеңіз.

3. **С кезеңіне көтерілу (қатаң PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` гистограммаларының 1,0 тенденциясын тексеріңіз.
   - Қоңырау есептегішінің тегіс күйде екенін растаңыз; әйтпесе төмендету қадамдарын орындаңыз.

## Демоционалды төмендету / бұрғылау

1. **Синтетикалық PQ тапшылығын туғызыңыз**

   Қорғаушы каталогын тек классикалық жазбаларға кесу арқылы ойын алаңындағы PQ релелерін өшіріңіз, содан кейін оркестр кэшін қайта жүктеңіз:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Қоңырау телеметриясын бақылаңыз**

   - Бақылау тақтасы: «Қоңырау жылдамдығы» тақтасы 0-ден жоғары.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` `anonymity_outcome="brownout"` `anonymity_reason="missing_majority_pq"` көмегімен хабарлауы керек.

3. **В кезеңіне / A кезеңіне төмендету**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Егер PQ жеткізілімі әлі де жеткіліксіз болса, `anon-guard-pq` деңгейіне төмендетіңіз. Бұрғылау есептегіштер орныққан кезде аяқталады және акцияларды қайта қолдануға болады.

4. **Қорғау каталогын қалпына келтіру**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметрия және артефактілер

- **Бақылау тақтасы:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus ескертулері:** `sorafs_orchestrator_policy_events_total` өшіру ескертуінің конфигурацияланған SLO (кез келген 10 минуттық терезеде <5%) төмен болуын қамтамасыз етіңіз.
- **Оқиға журналы:** түсірілген телеметриялық үзінділер мен оператор жазбаларын `docs/examples/soranet_pq_ratchet_fire_drill.log` файлына қосыңыз.
- **Қол қойылған түсіру:** бұрғылау журналы мен көрсеткіштер тақтасын `artifacts/soranet_pq_rollout/<timestamp>/` ішіне көшіру, BLAKE3 дайджесттерін есептеу және қол қойылған `rollout_capture.json` жасау үшін `cargo xtask soranet-rollout-capture` пайдаланыңыз.

Мысалы:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Жасалған метадеректер мен қолтаңбаны басқару пакетіне тіркеңіз.

## Кері қайтару

Егер жаттығу нақты PQ тапшылығын анықтаса, A сатысында қалып, Networking TL-ге хабарлаңыз және жиналған көрсеткіштерді және қорғау каталогының айырмашылықтарын оқиғаны бақылау құралына тіркеңіз. Қалыпты қызметті қалпына келтіру үшін бұрын түсірілген қорғаныс каталогын экспорттауды пайдаланыңыз.

:::tip Регрессияны қамту
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` осы бұрғылауға қолдау көрсететін синтетикалық тексеруді қамтамасыз етеді.
:::