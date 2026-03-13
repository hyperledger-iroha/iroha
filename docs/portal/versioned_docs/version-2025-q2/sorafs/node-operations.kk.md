---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations-kk
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
slug: /sorafs/node-operations-kk
---

:::ескерту Канондық дереккөз
Айналар `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Екі көшірмені де шығарылымдар бойынша туралаңыз.
:::

## Шолу

Бұл runbook операторларды Torii ішіндегі ендірілген `sorafs-node` орналастыруын тексеру арқылы көрсетеді. Әрбір бөлім тікелей SF-3 жеткізілімдерімен салыстырылады: пин/алу айналмалы сапарлар, қалпына келтіруді қайта бастау, квотаны қабылдамау және PoR үлгісін алу.

## 1. Пререквизиттер

- `torii.sorafs.storage` ішінде сақтау қызметкерін қосыңыз:

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

- Torii процесінің `data_dir` жүйесіне оқу/жазу рұқсаты бар екеніне көз жеткізіңіз.
- Декларация жазылғаннан кейін түйіннің күтілетін сыйымдылықты `GET /v2/sorafs/capacity/state` арқылы жарнамалайтынын растаңыз.
- Тегістеу қосулы кезде, бақылау тақталары нүкте мәндерімен қатар дірілсіз трендтерді бөлектеу үшін шикі және тегістелген GiB·сағ/PoR есептегіштерін көрсетеді.

### CLI Dry Run (қосымша)

HTTP соңғы нүктелерін ашпас бұрын жинақталған CLI көмегімен сақтау серверінің саулығын тексеруге болады.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Пәрмендер Norito JSON қорытындыларын басып шығарады және бөлік профилі немесе дайджест сәйкессіздіктерінен бас тартады, бұл оларды Torii сымдарынан бұрын CI түтінін тексеру үшін пайдалы етеді.【crates/sorafs_node/tests/cli.rs#L1】

Torii қосылғаннан кейін HTTP арқылы бірдей артефактілерді алуға болады:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Екі соңғы нүктеге ендірілген сақтау қызметкері қызмет көрсетеді, сондықтан CLI түтін сынақтары мен шлюз зондтары синхрондалады.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#1】L

## 2. Бекіту → Екі жаққа сапарды алу

1. Манифест + пайдалы жүктеме бумасын жасаңыз (мысалы, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` арқылы).
2. Манифестті base64 кодтауымен жіберіңіз:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON сұрауында `manifest_b64` және `payload_b64` болуы керек. Сәтті жауап `manifest_id_hex` және пайдалы жүктеме дайджестін қайтарады.
3. Бекітілген деректерді алыңыз:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-`data_b64` өрісін декодтаңыз және оның бастапқы байттарға сәйкес келетінін тексеріңіз.

## 3. Қалпына келтіру бұрғысын қайта іске қосыңыз

1. Жоғарыдағыдай кем дегенде бір манифестті бекітіңіз.
2. Torii процесін (немесе бүкіл түйінді) қайта іске қосыңыз.
3. Алу сұрауын қайта жіберіңіз. Пайдалы жүктеме әлі де алынуы керек және қайтарылған дайджест қайта іске қосу алдындағы мәнге сәйкес келуі керек.
4. `bytes_used` қайта жүктеуден кейінгі тұрақты манифесттерді көрсететінін растау үшін `GET /v2/sorafs/storage/state` тексеріңіз.

## 4. Квотадан бас тарту сынағы

1. `torii.sorafs.storage.max_capacity_bytes` мәнін шағын мәнге уақытша төмендетіңіз (мысалы, жалғыз манифест өлшемі).
2. Бір манифестті бекітіңіз; сұрау сәтті болуы керек.
3. Ұқсас өлшемдегі екінші манифестті бекіту әрекеті. Torii сұрауды HTTP `400` және құрамында `storage capacity exceeded` бар қате туралы хабарды қабылдамау керек.
4. Аяқтаған кезде қалыпты сыйымдылық шегін қалпына келтіріңіз.

## 5. PoR үлгісін алу зонды

1. Манифестті бекітіңіз.
2. PoR үлгісін сұрау:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Жауапта сұралған санау бар `samples` бар екенін және әрбір дәлелде сақталған манифест түбіріне қарсы тексерілетінін тексеріңіз.

## 6. Автоматтандыру ілмектері

- CI / түтін сынақтары қосылған мақсатты тексерулерді қайта пайдалана алады:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```ол `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` және `por_sampling_returns_verified_proofs` қамтиды.
- Бақылау тақталары мыналарды қадағалауы керек:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` және `torii_sorafs_storage_fetch_inflight`
  - PoR сәтті/сәтсіздік есептегіштері `/v2/sorafs/capacity/state` арқылы пайда болды
  - `sorafs_node_deal_publish_total{result=success|failure}` арқылы есеп айырысуды жариялау әрекеттері

Осы жаттығуларды орындау ендірілген жад қызметкерінің деректерді қабылдауына, қайта іске қосудан аман қалуына, конфигурацияланған квоталарды құрметтеуге және түйін кеңірек желіге сыйымдылықты жарияламас бұрын детерминирленген PoR дәлелдерін жасауға мүмкіндік береді.
