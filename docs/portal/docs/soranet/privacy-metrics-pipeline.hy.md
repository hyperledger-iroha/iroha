---
lang: hy
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9eafd2f3786dbe0ccfa7db2ec138d1d91e306e1691fbee801ba04a4165131655
source_last_modified: "2026-01-05T09:28:11.915061+00:00"
translation_last_reviewed: 2026-02-07
id: privacy-metrics-pipeline
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
:::

# SoraNet Privacy Metrics Pipeline

SNNet-8-ը ներկայացնում է գաղտնիության մասին տեղեկացված հեռաչափական մակերես ռելեի գործարկման ժամանակի համար: Այն
ռելեն այժմ միավորում է ձեռքսեղմման և շրջանային իրադարձությունները րոպեական չափի դույլերի մեջ և
արտահանում է միայն կոպիտ Prometheus հաշվիչներ՝ պահպանելով առանձին սխեմաներ
անհասանելի է, մինչդեռ օպերատորներին տալիս է տեսանելիություն:

## Ագրեգատորի ակնարկ

- Գործարկման ժամանակի իրականացումը գործում է `tools/soranet-relay/src/privacy.rs`-ում որպես
  `PrivacyAggregator`.
- Դույլերը միացված են պատի ժամացույցի րոպեով (`bucket_secs`, լռելյայն 60 վայրկյան) և
  պահվում է սահմանափակ օղակում (`max_completed_buckets`, լռելյայն 120): Կոլեկցիոներ
  բաժնետոմսերը պահպանում են իրենց սահմանափակված հետքաշումները (`max_share_lag_buckets`, լռելյայն 12)
  Այսպիսով, Prio-ի հնացած պատուհանները լվացվում են որպես ճնշված դույլեր, այլ ոչ թե արտահոսում
  հիշողություն կամ դիմակավոր խրված կոլեկտորներ:
- `RelayConfig::privacy`-ը քարտեզագրում է ուղիղ `PrivacyConfig`-ում՝ բացահայտելով թյունինգը
  բռնակներ (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`): Արտադրության գործարկման ժամանակը պահպանում է կանխադրվածները, մինչդեռ SNNet-8a-ն
  ներկայացնում է անվտանգ ագրեգացման շեմեր:
- Runtime մոդուլները գրանցում են իրադարձությունները մուտքագրված օգնականների միջոցով.
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes` և `record_gar_category`:

## Ռելե ադմինիստրատորի վերջնակետ

Օպերատորները կարող են հարցումներ կատարել ռելեի ադմինիստրատորի ունկնդիրից՝ չմշակված դիտարկումների միջոցով
`GET /privacy/events`. Վերջնական կետը վերադարձնում է նոր տողով սահմանազատված JSON
(`application/x-ndjson`), որը պարունակում է `SoranetPrivacyEventV1` օգտակար բեռներ՝ արտացոլված
ներքին `PrivacyEventBuffer`-ից: Բուֆերը պահպանում է նորագույն իրադարձությունները
դեպի `privacy.event_buffer_capacity` գրառումները (կանխադրված 4096) և ցամաքվում է
կարդալ, այնպես որ քերիչները պետք է բավական հաճախակի հարցում կատարեն՝ բացթողումներից խուսափելու համար: Իրադարձությունները ներառում են
նույն ձեռքսեղմում, շնչափող, ստուգված թողունակություն, ակտիվ միացում և GAR ազդանշաններ
որոնք սնուցում են Prometheus հաշվիչները՝ թույլ տալով ներքևում գտնվող կոլեկտորներին արխիվացնել
գաղտնիության համար անվտանգ հացահատիկներ կամ կերակրել անվտանգ ագրեգացման աշխատանքային հոսքեր:

## Ռելեի կոնֆիգուրացիա

Օպերատորները կարգավորում են գաղտնիության հեռաչափության կադրերը ռելեի կազմաձևման ֆայլում միջոցով
`privacy` բաժինը.

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Դաշտի լռելյայնները համապատասխանում են SNNet-8 բնութագրին և վավերացվում են բեռնման ժամանակ.

| Դաշտային | Նկարագրություն | Կանխադրված |
|-------|-------------|---------|
| `bucket_secs` | Յուրաքանչյուր ագրեգացիոն պատուհանի լայնությունը (վայրկյան): | `60` |
| `min_handshakes` | Աջակցողների նվազագույն քանակը, նախքան դույլը կարող է հաշվիչներ արձակել: | `12` |
| `flush_delay_buckets` | Լրացված դույլեր սպասելու համար, նախքան ողողելու փորձը: | `1` |
| `force_flush_buckets` | Առավելագույն տարիքը, նախքան մենք արտանետում ենք ճնշված դույլ: | `6` |
| `max_completed_buckets` | Պահպանված շերեփի կուտակում (կանխում է անսահմանափակ հիշողությունը): | `120` |
| `max_share_lag_buckets` | Կոլեկցիոների բաժնետոմսերի պահպանման պատուհանը մինչև ճնշելը: | `12` |
| `expected_shares` | Prio կոլեկցիոների բաժնետոմսերը պահանջվում են նախքան համատեղելը: | `2` |
| `event_buffer_capacity` | NDJSON իրադարձությունների հետադարձ գրառում ադմինիստրատորի հոսքի համար: | `4096` |

Սահմանելով `force_flush_buckets` ավելի ցածր, քան `flush_delay_buckets`, զրոյացնելով
շեմերը, կամ պահպանման պահակախմբի անջատումն այժմ ձախողվում է վավերացումից խուսափելու համար
տեղակայումներ, որոնք կարող են արտահոսել մեկ ռելեի հեռաչափություն:

`event_buffer_capacity` սահմանաչափը նաև սահմանափակում է `/admin/privacy/events`՝ ապահովելով.
քերիչները չեն կարող անորոշ ժամանակով հետ մնալ:

## Prio կոլեկցիոների բաժնետոմսեր

SNNet-8a-ն տեղադրում է երկակի կոլեկցիոներներ, որոնք արտանետում են գաղտնի ընդհանուր Prio դույլեր: Այն
նվագախումբն այժմ վերլուծում է `/privacy/events` NDJSON հոսքը երկուսի համար
`SoranetPrivacyEventV1` գրառումներ և `SoranetPrivacyPrioShareV1` բաժնետոմսեր,
դրանք ուղարկելով `SoranetSecureAggregator::ingest_prio_share`: Դույլերը արտանետում են
`PrivacyBucketConfig::expected_shares` ներդրումները ժամանելուց հետո՝ արտացոլելով այն
ռելեի վարքագիծը. Բաժնետոմսերը վավերացվում են դույլերի հավասարեցման և հիստոգրամի ձևի համար
նախքան `SoranetPrivacyBucketMetricsV1`-ում միավորելը: Եթե համակցված
ձեռքսեղմման թիվը ընկնում է `min_contributors`-ից ցածր, դույլն արտահանվում է որպես
`suppressed`՝ արտացոլելով ռելեային ագրեգատորի վարքը: Ճնշված
Windows-ն այժմ թողարկում է `suppression_reason` պիտակը, որպեսզի օպերատորները կարողանան տարբերակել
`insufficient_contributors`, `collector_suppressed` միջև,
`collector_window_elapsed` և `forced_flush_window_elapsed` սցենարները, երբ
հեռաչափության բացերի ախտորոշում. `collector_window_elapsed` պատճառը նույնպես կրակում է
երբ Prio-ի բաժնետոմսերը մնում են `max_share_lag_buckets`-ի կողքին՝ ստեղծելով խրված կոլեկցիոներներ
տեսանելի է առանց հիշողության մեջ հնացած կուտակիչներ թողնելու:

## Torii Կլանման վերջնակետեր

Torii-ն այժմ բացահայտում է երկու HTTP վերջնակետեր, որոնք փակված են հեռաչափով, այնպես որ ռելեներ և կոլեկտորներ
կարող է ուղարկել դիտարկումներ՝ առանց պատվիրված տրանսպորտի ներդրման.

- `POST /v2/soranet/privacy/event` ընդունում է ա
  `RecordSoranetPrivacyEventDto` օգտակար բեռ: Մարմինը փաթաթում ա
  `SoranetPrivacyEventV1` գումարած կամընտիր `source` պիտակը: Torii-ը վավերացնում է
  հարցումը ակտիվ հեռաչափության պրոֆիլի դեմ, գրանցում է իրադարձությունը և պատասխանում
  HTTP `202 Accepted`-ի հետ միասին Norito JSON ծրարի հետ, որը պարունակում է
  հաշվարկված դույլի պատուհան (`bucket_start_unix`, `bucket_duration_secs`) և
  ռելեի ռեժիմ:
- `POST /v2/soranet/privacy/share` ընդունում է `RecordSoranetPrivacyShareDto`
  օգտակար բեռ. Թափքը կրում է `SoranetPrivacyPrioShareV1` և կամընտիր
  `forwarded_by` հուշում է, որպեսզի օպերատորները կարողանան ստուգել կոլեկտորների հոսքերը: Հաջողակ
  ներկայացումները վերադարձնում են HTTP `202 Accepted` Norito JSON ծրարով ամփոփող
  կոլեկցիոները, դույլի պատուհանը և զսպման հուշում; վավերացման ձախողումների քարտեզը
  `Conversion` հեռաչափական պատասխան՝ դետերմինիստական սխալների կառավարումը պահպանելու համար
  կոլեկցիոներների միջով: Նվագախմբի իրադարձությունների հանգույցն այժմ թողարկում է այս բաժնետոմսերը
  հարցումների ռելեներ՝ պահելով Torii-ի Prio ակումուլյատորը միացված ռելեի դույլերի հետ համաժամանակյա:

Երկու վերջնակետերն էլ հարգում են հեռաչափության պրոֆիլը. նրանք թողարկում են `503 ծառայություն
Անհասանելի է, երբ չափիչները անջատված են: Հաճախորդները կարող են ուղարկել Norito երկուական տարբերակ
(`application/x.norito`) կամ Norito JSON (`application/x.norito+json`) մարմիններ;
սերվերը ավտոմատ կերպով բանակցում է ձևաչափի մասին ստանդարտ Torii-ի միջոցով
արդյունահանողներ.

## Prometheus չափումներ

Յուրաքանչյուր արտահանվող դույլ կրում է `mode` (`entry`, `middle`, `exit`) և
`bucket_start` պիտակներ. Արտանետվում են հետևյալ մետրային ընտանիքները.

| Մետրական | Նկարագրություն |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Ձեռքսեղմման տաքսոնոմիա `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`-ով: |
| `soranet_privacy_throttles_total{scope}` | Շնչափող հաշվիչներ `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`-ով: |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Սառեցման ընդհանուր տևողությունները նպաստում են սեղմված ձեռքսեղմումներին: |
| `soranet_privacy_verified_bytes_total` | Ստուգված թողունակությունը կույր չափման ապացույցներից: |
| `soranet_privacy_active_circuits_{avg,max}` | Միջին և գագաթնակետային ակտիվ սխեմաներ մեկ դույլի համար: |
| `soranet_privacy_rtt_millis{percentile}` | RTT տոկոսային հաշվարկներ (`p50`, `p90`, `p99`): |
| `soranet_privacy_gar_reports_total{category_hash}` | Հաշված կառավարման գործողությունների հաշվետվության հաշվիչներ՝ ըստ կատեգորիաների ամփոփման: |
| `soranet_privacy_bucket_suppressed` | Դույլերը պահվել են, քանի որ ներդրման շեմը չի պահպանվել: |
| `soranet_privacy_pending_collectors{mode}` | Կոլեկցիոների բաժնետոմսերի կուտակիչները սպասող համակցություն, խմբավորված ըստ ռելեի ռեժիմի: |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`-ով փակված դույլերի հաշվիչները, որպեսզի վահանակները կարողանան վերագրել գաղտնիության բացերը: |
| `soranet_privacy_snapshot_suppression_ratio` | Վերջին արտահոսքի ճնշված/թափված հարաբերակցությունը (0–1), օգտակար է զգուշավոր բյուջեների համար: |
| `soranet_privacy_last_poll_unixtime` | UNIX-ի ամենավերջին հաջողված հարցման ժամանակի դրոշմը (առաջացնում է կոլեկցիոների անգործության մասին ահազանգը): |
| `soranet_privacy_collector_enabled` | Չափիչ, որը շրջվում է դեպի `0`, երբ գաղտնիության հավաքիչն անջատված է կամ չի գործարկվում (հաստատում է կոլեկցիոների հաշմանդամության մասին ահազանգը): |
| `soranet_privacy_poll_errors_total{provider}` | Քվեարկության ձախողումները խմբավորված ըստ ռելեի կեղծանունների (վերծանման սխալների ավելացում, HTTP ձախողումներ կամ անսպասելի կարգավիճակի կոդեր): |

Դույլերն առանց դիտարկումների մնում են լուռ՝ առանց պահելու վահանակները կոկիկ
զրոյական լիցքավորված պատուհանների պատրաստում:

## Գործառնական ուղեցույց

1. **Dashboards** – գծեք վերը նշված չափումները՝ խմբավորված ըստ `mode` և `window_start`:
   Ընդգծեք բացակայող պատուհանները մակերեսային կոլեկտորի կամ ռելեի հետ կապված խնդիրները: Օգտագործեք
   `soranet_privacy_suppression_total{reason}` տարբերակել ներդրողին
   բացթողումներ կոլեկցիոների վրա հիմնված ճնշումից բացթողումներ: Grafana
   ակտիվն այժմ առաքում է հատուկ **«Զսպման պատճառները (5մ)»**, որը սնվում է այդ անձանց կողմից
   հաշվիչներ գումարած **«Զսպված դույլ %»** վիճակագրություն, որը հաշվարկում է
   `sum(soranet_privacy_bucket_suppressed) / count(...)` ըստ ընտրության
   օպերատորները կարող են մի հայացքով նկատել բյուջեի խախտումները: **Կոլեկցիոների մասնաբաժինը
   Backlog** շարքը (`soranet_privacy_pending_collectors`) և **Snapshot
   Ճնշման հարաբերակցությունը** վիճակագրությունը ընդգծում է խրված կոլեկտորները և բյուջեի շեղումը ընթացքում
   ավտոմատացված վազք.
2. **Զգուշացում** – տագնապներ վարեք գաղտնիության համար ապահով հաշվիչներից.
   սառեցման հաճախականությունը, RTT դրեյֆը և հզորության մերժումը: Քանի որ հաշվիչներն են
   միապաղաղ յուրաքանչյուր դույլի մեջ, արագության վրա հիմնված պարզ կանոնները լավ են աշխատում:
3. **Միջադեպի արձագանքը** – նախ ապավինեք ագրեգացված տվյալներին: Երբ ավելի խորը կարգաբերում է
   անհրաժեշտ է, պահանջեք ռելեներ՝ վերարտադրելու դույլի նկարահանումները կամ ստուգելու կույրը
   Չափման ապացույցներ՝ չմշակված երթևեկության տեղեկամատյաններ հավաքելու փոխարեն:
4. **Պահպանում** – բավական հաճախ քերել՝ գերազանցելուց խուսափելու համար
   `max_completed_buckets`. Արտահանողները պետք է վերաբերվեն Prometheus արտադրանքին որպես
   կանոնական աղբյուր և թողնել տեղական դույլերը մեկ անգամ փոխանցելուց հետո:

## Suppression Analytics & Automated RunsSNNet-8-ի ընդունումը կախված է ցույց տալով, որ ավտոմատացված կոլեկտորները մնում են
առողջ, և այդ ճնշումը մնում է քաղաքականության սահմաններում (≤10% դույլերի մեկ
փոխանցել ցանկացած 30 րոպեանոց պատուհանի վրա): Գործիքավորումն անհրաժեշտ էր այդ դարպասին այժմ բավարարելու համար
նավեր ծառի հետ; օպերատորները պետք է այն մտցնեն իրենց շաբաթական ծեսերի մեջ: Նորը
Grafana ճնշող պանելները արտացոլում են ներքևում գտնվող PromQL հատվածները՝ տալով անընդմեջ
թիմերի կենդանի տեսանելիությունը, նախքան նրանք պետք է վերադառնան ձեռքով հարցումներին:

### PromQL բաղադրատոմսեր ճնշելու վերանայման համար

Օպերատորները պետք է ձեռքի տակ պահեն հետևյալ PromQL օգնականները. երկուսն էլ նշված են
ընդհանուր Grafana վահանակում (`dashboards/grafana/soranet_privacy_metrics.json`)
և Alertmanager կանոնները.

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Օգտագործեք ելքային հարաբերակցությունը՝ հաստատելու համար, որ **«Suppressed Bucket %»** վիճակագրությունը մնում է ստորև
քաղաքականության բյուջեն; արագ արձագանքելու համար ցցերի դետեկտորը միացրեք Alertmanager-ին
երբ ներդրողների թվերն անսպասելիորեն նվազում են:

### Անցանց դույլի հաշվետվություն CLI

Աշխատանքային տարածքը բացահայտում է `cargo xtask soranet-privacy-report` մեկանգամյա NDJSON-ի համար
գրավում է. Ուղղեք այն մեկ կամ մի քանի ռելեի ադմինիստրատորի արտահանման վրա.

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Օգնականը հեռարձակում է գրավումը `SoranetSecureAggregator`-ի միջոցով, տպում ա
suppression-ի ամփոփագիր stdout-ին և ընտրովի գրում է JSON-ի կառուցվածքային հաշվետվություն
`--json-out <path|->`-ի միջոցով: Այն հարգում է նույն կոճակները, ինչ կենդանի կոլեկցիոները
(`--bucket-secs`, `--min-contributors`, `--expected-shares` և այլն), վարձով
օպերատորները վերարտադրում են պատմական նկարահանումները տարբեր շեմերի տակ, երբ եռապատկվում են
մի հարց. Կցեք JSON-ը Grafana սքրինշոթերի կողքին, որպեսզի SNNet-8-ը
suppression analytics gate-ը մնում է ստուգման ենթակա:

### Առաջին ավտոմատացված գործարկման ստուգաթերթը

Կառավարումը դեռևս պահանջում է ապացուցել, որ առաջին ավտոմատացման գործարկումը բավարարել է
ճնշող բյուջե. Օգնողն այժմ ընդունում է `--max-suppression-ratio <0-1>` այսպես
CI կամ օպերատորները կարող են արագ ձախողվել, երբ ճնշված դույլերը գերազանցում են թույլատրվածը
պատուհան (կանխադրված 10%) կամ երբ դույլեր դեռ չկան: Առաջարկվող հոսք.

1. Արտահանեք NDJSON ռելեի ադմինիստրատորի վերջնակետ(ներ)ից, գումարած նվագախմբի կողմից
   `/v2/soranet/privacy/event|share` հոսք դեպի
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Գործարկել օգնականը քաղաքականության բյուջեով.

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Հրամանը տպում է դիտարկված հարաբերակցությունը և դուրս է գալիս զրոյից, երբ բյուջեն է
   գերազանցել է **կամ**, երբ դույլեր պատրաստ չեն, ինչը ցույց է տալիս, որ հեռաչափությունը չի պատրաստվել
   դեռ արտադրվել է վազքի համար: Կենդանի չափումները պետք է ցույց տան
   `soranet_privacy_pending_collectors` արտահոսք դեպի զրոյի և
   `soranet_privacy_snapshot_suppression_ratio` մնալը նույն բյուջեի տակ
   մինչ վազքը կատարվում է:
3. Արխիվացրեք JSON ելքը և CLI գրանցամատյանը SNNet-8 ապացույցների փաթեթով նախքան
   շրջել տրանսպորտի լռելյայն, որպեսզի վերանայողները կարողանան վերարտադրել ճշգրիտ արտեֆակտները:

## Հաջորդ քայլերը (SNNet-8a)

- Ինտեգրել երկակի Prio կոլեկցիոներները՝ միացնելով դրանց մասնաբաժնի ընդունումը
  աշխատանքի ժամանակը, այնպես որ ռելեներն ու կոլեկտորները թողարկեն հետևողական `SoranetPrivacyBucketMetricsV1`
  օգտակար բեռներ. *(Կատարված է՝ տես `ingest_privacy_payload` in
  `crates/sorafs_orchestrator/src/lib.rs` և ուղեկցող թեստեր։)*
- Հրապարակեք ընդհանուր Prometheus վահանակը JSON և զգուշացման կանոններ
  ճնշելու բացերը, կոլեկցիոների առողջությունը և անանունության խախտումները: *(Կատարված է, տես
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml` և վավերացման սարքեր։)*
- Արտադրեք դիֆերենցիալ-գաղտնիության չափորոշիչ արտեֆակտներ, որոնք նկարագրված են
  `privacy_metrics_dp.md`, ներառյալ վերարտադրվող նոթատետրերը և կառավարումը
  մարսում է. *(Կատարված է՝ նոթատետր + արտեֆակտներ, որոնք ստեղծվել են
  `scripts/telemetry/run_privacy_dp.py`; CI փաթաթան
  `scripts/telemetry/run_privacy_dp_notebook.sh`-ը կատարում է նոթատետրը միջոցով
  `.github/workflows/release-pipeline.yml` աշխատանքային հոսք; կառավարման ամփոփագիր ներկայացվել է
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

Ընթացիկ թողարկումն ապահովում է SNNet-8 հիմքը.
Գաղտնիության համար անվտանգ հեռաչափություն, որն ուղղակիորեն տեղադրվում է գոյություն ունեցող Prometheus քերիչների մեջ
և վահանակներ: Գաղտնիության դիֆերենցիալ չափորոշիչ արտեֆակտներ կան, որոնք
թողարկման խողովակաշարի աշխատանքային հոսքը պահպանում է նոութբուքի արդյունքները թարմ, իսկ մնացածը
աշխատանքը կենտրոնանում է առաջին ավտոմատացված վազքի մոնիտորինգի վրա, ինչպես նաև ընդլայնելով ճնշումը
զգոն վերլուծություն.