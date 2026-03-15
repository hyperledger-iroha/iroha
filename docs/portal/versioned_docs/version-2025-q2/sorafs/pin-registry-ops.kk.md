---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops-kk
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
slug: /sorafs/pin-registry-ops-kk
---

:::ескерту Канондық дереккөз
Айналар `docs/source/sorafs/runbooks/pin_registry_ops.md`. Екі нұсқаны да шығарылымдар бойынша туралаңыз.
:::

## Шолу

Бұл runbook SoraFS PIN тізілімін және оның репликация қызметі деңгейі келісімдерін (SLAs) бақылау және триаждау жолын құжаттайды. Көрсеткіштер `iroha_torii` бастап келеді және `torii_sorafs_*` аттар кеңістігі астында Prometheus арқылы экспортталады. Torii тізілім күйін фондық режимде 30 секундтық интервалда таңдайды, сондықтан ешқандай оператор `/v2/sorafs/pin/*` соңғы нүктелерін сұрамаса да бақылау тақталары ағымдағы болып қалады. Төмендегі бөлімдерге тікелей салыстырылатын, пайдалануға дайын Grafana орналасуы үшін таңдалған бақылау тақтасын (`docs/source/grafana_sorafs_pin_registry.json`) импорттаңыз.

## Метрика сілтемесі

| метрикалық | Белгілер | Сипаттама |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Өмірлік цикл күйі бойынша тізбектегі манифестті түгендеу. |
| `torii_sorafs_registry_aliases_total` | — | Тізілімде жазылған белсенді манифест бүркеншік аттарының саны. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Күй бойынша сегменттелген репликация тәртібінің артта қалу тізімі. |
| `torii_sorafs_replication_backlog_total` | — | `pending` тапсырыстарын көрсететін ыңғайлылық көрсеткіші. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA есебі: `met` белгіленген мерзімде орындалған тапсырыстарды санайды, `missed` кеш аяқталуларды + жарамдылық мерзімін біріктіреді, `pending` орындалмаған тапсырыстарды көрсетеді. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Агрегацияланған аяқтау кідірісі (шығару мен аяқтау арасындағы дәуірлер). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Күтудегі тапсырыстың бос терезелері (соңғы мерзім минус шығарылған дәуір). |

Барлық өлшегіштер әрбір суретті түсіргенде бастапқы күйге қайтарылады, сондықтан бақылау тақталары `1m` каденсімен немесе жылдамырақ таңдалуы керек.

## Grafana бақылау тақтасы

JSON бақылау тақтасы оператордың жұмыс үрдістерін қамтитын жеті панельмен жеткізіледі. Сұраулар төменде берілген, егер сіз тапсырыс бойынша диаграммалар жасағыңыз келсе, жылдам анықтама алу үшін.

1. **Манифесттің өмірлік циклі** – `torii_sorafs_registry_manifests_total` (`status` бойынша топтастырылған).
2. ** Бүркеншік ат каталогының тренд** – `torii_sorafs_registry_aliases_total`.
3. **Күйі бойынша тапсырыс кезегі** – `torii_sorafs_registry_orders_total` (`status` бойынша топтастырылған).
4. **Баклог пен мерзімі өткен тапсырыстар** – беттің қанықтылығына `torii_sorafs_replication_backlog_total` және `torii_sorafs_registry_orders_total{status="expired"}` біріктіреді.
5. **SLA табыстылық коэффициенті** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Кідіріс пен соңғы мерзімнің босаңсуы** – қабаттасу `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` және `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Абсолютті бос еден қажет болғанда `min_over_time` көріністерін қосу үшін Grafana түрлендірулерін пайдаланыңыз, мысалы:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Жіберіп алған тапсырыстар (1 сағаттық мөлшерлеме)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Ескерту шектері- **SLA табысы  0**
  - Шекті: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Әрекет: провайдердің бұзылуын растау үшін басқару манифесттерін тексеріңіз.
- **Аяқтау p95 > соңғы мерзімнің орт.
  - Шекті: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Әрекет: провайдерлердің белгіленген мерзімнен бұрын жасағанын тексеріңіз; қайта тағайындаулар беруді қарастыру.

### Мысалы Prometheus ережелері

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Триаж жұмыс процесі

1. **Себепті анықтаңыз**
   - Егер SLA артта қалушылық төмен болып тұрғанда өсуді өткізіп алса, провайдердің өнімділігіне назар аударыңыз (PoR ақаулары, кеш аяқталу).
   - Егер артта қалу тұрақты жіберіп алумен өссе, кеңестің мақұлдауын күтіп тұрған манифесттерді растау үшін қабылдауды (`/v2/sorafs/pin/*`) тексеріңіз.
2. **Провайдер күйін растау**
   - `iroha app sorafs providers list` іске қосыңыз және жарнамаланған мүмкіндіктердің репликация талаптарына сәйкес келетінін тексеріңіз.
   - Қамтамасыз етілген GiB және PoR сәттілігін растау үшін `torii_sorafs_capacity_*` көрсеткіштерін тексеріңіз.
3. **Репликацияны қайта тағайындау**
   - Артта қалу (`stat="avg"`) 5 дәуірден төмен түскен кезде `sorafs_manifest_stub capacity replication-order` арқылы жаңа тапсырыстарды беріңіз (манифест/CAR қаптамасы `iroha app sorafs toolkit pack` пайдаланады).
   - Егер бүркеншік аттарда белсенді манифест байланыстары болмаса, басқаруды хабардар етіңіз (`torii_sorafs_registry_aliases_total` күтпеген жерден төмендейді).
4. **Құжаттаманың нәтижесі**
   - Уақыт белгілері мен әсер еткен манифест дайджесттері бар SoraFS операциялар журналында оқиға жазбаларын жазыңыз.
   - Жаңа сәтсіздік режимдері немесе бақылау тақталары енгізілген болса, осы жұмыс кітабын жаңартыңыз.

## Іске қосу жоспары

Өндірісте бүркеншік ат кэш саясатын қосу немесе қатайту кезінде осы кезеңдік процедураны орындаңыз:1. **Конфигурацияны дайындаңыз**
   - `torii.sorafs_alias_cache` нұсқасын `iroha_config` (пайдаланушы → нақты) келісілген TTL және жеңілдік терезелерімен жаңартыңыз: `positive_ttl`, `refresh_window`, `hard_expiry`, I108030, I18080 `revocation_ttl`, `rotation_max_age`, `successor_grace` және `governance_grace`. Әдепкі мәндер `docs/source/sorafs_alias_policy.md` саясатына сәйкес келеді.
   - SDK үшін бірдей мәндерді олардың конфигурация деңгейлері арқылы таратыңыз (Rust/NAPI/Python байланыстыруларындағы `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`), клиенттің орындауы шлюзге сәйкес келеді.
2. **Сахнадағы құрғақ жүгіріс**
   - Өндіріс топологиясын көрсететін кезеңдік кластерге конфигурация өзгертуін қолданыңыз.
   - `cargo xtask sorafs-pin-fixtures` іске қосыңыз канондық бүркеншік атын әлі де декодтауды растау және айналу; кез келген сәйкессіздік алдымен шешілуі керек жоғары ағынды манифест дрейфін білдіреді.
   - `/v2/sorafs/pin/{digest}` және `/v2/sorafs/aliases` соңғы нүктелерін жаңа, жаңарту терезесі, мерзімі өткен және мерзімі өтіп кеткен жағдайларды қамтитын синтетикалық дәлелдермен орындаңыз. HTTP күй кодтарын, тақырыптарды (`Sora-Proof-Status`, `Retry-After`, `Warning`) және JSON негізгі өрістерін осы жұмыс кітабына қарсы тексеріңіз.
3. **Өндірісте қосу**
   - Жаңа конфигурацияны стандартты өзгерту терезесі арқылы шығарыңыз. Алдымен оны Torii қолданбасына қолданыңыз, содан кейін түйін журналдардағы жаңа саясатты растағаннан кейін шлюздерді/SDK қызметтерін қайта іске қосыңыз.
   - `docs/source/grafana_sorafs_pin_registry.json` файлын Grafana ішіне импорттаңыз (немесе бар бақылау тақталарын жаңартыңыз) және бүркеншік аттың кэш жаңарту тақталарын NOC жұмыс кеңістігіне бекітіңіз.
4. **Орналастырудан кейінгі тексеру**
   - `torii_sorafs_alias_cache_refresh_total` және `torii_sorafs_alias_cache_age_seconds` мониторларын 30 минутқа созыңыз. `error`/`expired` қисық сызықтарындағы төбелер саясатты жаңарту терезелерімен корреляциялануы керек; күтпеген өсу операторлар жалғастырмас бұрын бүркеншік аттың дәлелдері мен провайдердің денсаулығын тексеруі керек дегенді білдіреді.
   - Клиенттік журналдар бірдей саясат шешімдерін көрсететінін растаңыз (дәлелдеу ескірген немесе мерзімі өткен кезде SDK қателерді көрсетеді). Клиент ескертулерінің болмауы қате конфигурацияны көрсетеді.
5. **Қолданбау**
   - Бүркеншік ат беру артта қалса және жаңарту терезесі жиі өшірілсе, конфигурацияда `refresh_window` және `positive_ttl` арттыру арқылы саясатты уақытша босаңсытып, қайта орналастырыңыз. `hard_expiry` толық сақтаңыз, сондықтан шынымен ескірген дәлелдер әлі қабылданбайды.
   - Алдыңғы `iroha_config` суретін қалпына келтіру арқылы алдыңғы конфигурацияға оралыңыз, егер телеметрия жоғарырақ `error` сандарын көрсетуді жалғастырса, одан кейін бүркеншік атын жасау кідірістерін қадағалау үшін оқиғаны ашыңыз.

## Қатысты материалдар

- `docs/source/sorafs/pin_registry_plan.md` — іске асыру жол картасы және басқару контексі.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — сақтау қызметкерінің әрекеттері, осы тізілім ойын кітабын толықтырады.
