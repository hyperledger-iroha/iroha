---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d99cea6198ef7ea6c75d7823854237983f17c3341cea2b3e491bb03e54531f2
source_last_modified: "2026-01-22T14:35:36.797300+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
Հայելիներ `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Պահպանեք երկու տարբերակները համաժամեցված, մինչև Sphinx հավաքածուն դուրս գա:
:::

## Տեսություն

Այս վազքագիրքը քայլում է օպերատորներին Torii-ի ներսում ներկառուցված `sorafs-node` տեղակայման վավերացման միջոցով: Յուրաքանչյուր բաժին ուղղակիորեն քարտեզագրվում է SF-3-ի արտադրանքներին. կապում/բերում է շրջանաձև ուղևորություններ, վերագործարկում վերականգնում, քվոտայի մերժում և PoR նմուշառում:

## 1. Նախադրյալներ

- Միացնել պահեստավորման աշխատողը `torii.sorafs.storage`-ում.

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Համոզվեք, որ Torii գործընթացն ունի կարդալու/գրելու մուտք դեպի `data_dir`:
- Հաստատեք, որ հանգույցը գովազդում է ակնկալվող հզորությունը `GET /v2/sorafs/capacity/state`-ի միջոցով, երբ հայտարարագիրը գրանցվի:
- Երբ հարթեցումը միացված է, վահանակները ցուցադրում են ինչպես հում, այնպես էլ հարթեցված GiB·hour/PoR հաշվիչները՝ կետային արժեքների հետ մեկտեղ ընդգծելու համար առանց ցնցումների միտումները:

### CLI Dry Run (ըստ ցանկության)

Նախքան HTTP-ի վերջնակետերը բացահայտելը, դուք կարող եք խելամտորեն ստուգել պահեստային տարածքը միացված CLI-ով:【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

Հրամանները տպում են Norito JSON ամփոփագրեր և մերժում են պրոֆիլների կամ բովանդակության անհամապատասխանությունները՝ դրանք օգտակար դարձնելով CI ծխի ստուգման համար՝ Torii լարերից առաջ։【crates/sorafs_node/tests/cli.rs#L1】

### PoR Proof Rehearsal

Օպերատորներն այժմ կարող են վերարտադրել կառավարման կողմից թողարկված PoR արտեֆակտները տեղական մակարդակում՝ նախքան դրանք Torii վերբեռնելը: CLI-ն նորից օգտագործում է նույն `sorafs-node` մուտքի ուղին, այնպես որ տեղական գործարկումները բացահայտում են վավերացման ճշգրիտ սխալները, որոնք կվերադարձնի HTTP API-ն:

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Հրամանը թողարկում է JSON ամփոփագիր (մանիֆեստի ամփոփում, մատակարարի id, ապացույցների ամփոփում, նմուշների քանակ, կամընտիր դատավճռի արդյունք): Տրամադրեք `--manifest-id=<hex>`՝ համոզվելու համար, որ պահված մանիֆեստը համապատասխանում է մարտահրավերների ամփոփմանը, և `--json-out=<path>`, երբ ցանկանում եք արխիվացնել ամփոփագիրը բնօրինակ արտեֆակտներով՝ աուդիտորական ապացույցների համար: Ներառյալ `--verdict`-ը թույլ է տալիս փորձարկել ամբողջ մարտահրավերը → ապացույց → դատավճիռը անցանց ռեժիմում՝ նախքան HTTP API-ն զանգահարելը:

Երբ Torii-ը ուղիղ եթերում է, դուք կարող եք առբերել նույն արտեֆակտները HTTP-ի միջոցով.

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Երկու վերջնակետերն էլ սպասարկվում են ներկառուցված պահեստավորման աշխատողի կողմից, ուստի CLI ծխի թեստերը և դարպասի զոնդերը մնում են համաժամանակյա։【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1

## 2. Ամրացրեք → Ստացեք հետադարձ ուղևորություն

1. Ստեղծեք մանիֆեստ + օգտակար բեռի փաթեթ (օրինակ՝ `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`-ով):
2. Ներկայացրե՛ք մանիֆեստը base64 կոդավորմամբ.

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON հարցումը պետք է պարունակի `manifest_b64` և `payload_b64`: Հաջող պատասխանը վերադարձնում է `manifest_id_hex`-ը և օգտակար բեռի ամփոփումը:
3. Վերցրեք ամրացված տվյալները.

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-վերծանեք `data_b64` դաշտը և ստուգեք, որ այն համապատասխանում է բնօրինակ բայթերին:

## 3. Վերագործարկեք վերականգնման փորվածքը

1. Ամրացրեք առնվազն մեկ մանիֆեստ ինչպես վերևում:
2. Վերագործարկեք Torii գործընթացը (կամ ամբողջ հանգույցը):
3. Կրկին ներկայացրեք առբերման հարցումը: Օգտակար բեռը դեռ պետք է վերականգնվի, և վերադարձված ամփոփումը պետք է համապատասխանի նախնական վերագործարկման արժեքին:
4. Ստուգեք `GET /v2/sorafs/storage/state`-ը՝ հաստատելու համար, որ `bytes_used`-ն արտացոլում է վերագործարկումից հետո պահպանվող դրսևորումները:

## 4. Քվոտայի մերժման թեստ

1. Ժամանակավորապես իջեցրեք `torii.sorafs.storage.max_capacity_bytes`-ը փոքր արժեքի (օրինակ՝ մեկ մանիֆեստի չափի):
2. Ամրացրեք մեկ մանիֆեստ; հարցումը պետք է հաջողվի:
3. Փորձեք ամրացնել նմանատիպ չափի երկրորդ մանիֆեստը: Torii-ը պետք է մերժի հարցումը HTTP `400`-ով և սխալի հաղորդագրությունով, որը պարունակում է `storage capacity exceeded`:
4. Ավարտելուց հետո վերականգնեք նորմալ հզորության սահմանաչափը:

## 5. Պահպանում / GC ստուգում (միայն կարդալու)

1. Գործարկեք տեղային պահպանման սկան՝ պահեստավորման գրացուցակի դեմ՝

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Ստուգեք միայն ժամկետանց մանիֆեստները (միայն չոր գործարկում, ջնջումներ չկան).

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. Օգտագործեք `--now` կամ `--grace-secs`՝ գնահատման պատուհանը ամրացնելու համար, երբ համեմատում եք հաշվետվությունները սերվերների կամ միջադեպերի միջև:

GC CLI-ն միտումնավոր է միայն կարդալու համար: Օգտագործեք այն՝ աուդիտի հետքերի պահպանման ժամկետները և ժամկետանց դրսևորվող գույքագրումը գրանցելու համար. մի հեռացրեք տվյալները ձեռքով արտադրության մեջ:

## 6. PoR նմուշառման զոնդ

1. Ամրացրեք մանիֆեստը:
2. Պահանջել PoR նմուշ.

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Ստուգեք, որ պատասխանը պարունակում է `samples` պահանջվող քանակով, և որ յուրաքանչյուր ապացույց վավերացված է պահված մանիֆեստի արմատի հետ:

## 7. Ավտոմատացման Կեռիկներ

- CI / ծխի թեստերը կարող են կրկին օգտագործել նպատակային ստուգումները, որոնք ավելացվել են.

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  որը ներառում է `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` և `por_sampling_returns_verified_proofs`:
- Վահանակները պետք է հետևեն.
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` և `torii_sorafs_storage_fetch_inflight`
  - PoR-ի հաջողության/ձախողման հաշվիչները հայտնվել են `/v2/sorafs/capacity/state`-ի միջոցով
  - Կարգավորման հրապարակման փորձերը `sorafs_node_deal_publish_total{result=success|failure}`-ի միջոցով

Այս զորավարժություններին հետևելը երաշխավորում է, որ ներկառուցված պահեստի աշխատողը կարող է կուլ տալ տվյալներ, գոյատևել վերագործարկումից, հարգել կազմաձևված քվոտաները և առաջացնել որոշիչ PoR ապացույցներ, նախքան հանգույցը գովազդում է հզորությունը ավելի լայն ցանցում: