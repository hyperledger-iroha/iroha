---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
མེ་ལོང་ `docs/source/sorafs/runbooks/sorafs_node_ops.md`. འདྲ་བཤུས་གཉིས་ཆ་ར་ གསར་བཏོན་ཚུ་ནང་ ཕྲང་སྒྲིག་འབད་བཞག།
:::

## སྤྱི་མཐོང་།

རན་དེབ་འདི་གིས་ བཀོལ་སྤྱོད་པ་ཚུ་ལུ་ `sorafs-node` བཀྲམ་སྤེལ་ཅིག་ བདེན་དཔྱད་འབད་དེ་ Torii ནང་འཁོད་ལུ་ བདེན་དཔྱད་འབདཝ་ཨིན། དབྱེ་ཚན་རེ་རེ་གིས་ ཨེསི་ཨེཕ་-༣ ལུ་ ཐད་ཀར་དུ་ སབ་ཁྲ་བཟོཝ་ཨིན།

## 1. སྔོན་འགྲོ།

- `torii.sorafs.storage` ནང་ གསོག་འཇོག་ལས་བྱེད་པ་འདི་ལྕོགས་ཅན་བཟོ།

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

- Torii བྱ་རིམ་འདི་ `data_dir` ལུ་ལྷག་ནི་/འབྲི་ནི་ཡོདཔ་ངེས་གཏན་བཟོ།
- གསལ་བསྒྲགས་ཅིག་ཐོ་བཀོད་འབད་ཚར་བའི་ཤུལ་ལས་ `GET /v1/sorafs/capacity/state` བརྒྱུད་དེ་ མཐུད་མཚམས་ཁྱབ་བསྒྲགས་འདི་ ངེས་འཛིན་འབདཝ་ཨིན།
- འཇམ་ཐང་ཐང་བཟོ་ནི་འདི་ ལྕོགས་ཅན་བཟོ་བའི་སྐབས་ ཌེཤ་བོརཌ་ཚུ་གིས་ གིབི་·ཆུ་ཚོད་/པོ་ཨར་ ཀའུན་ཊར་གཉིས་ཆ་ར་ གསལ་སྟོན་འབདཝ་ཨིན།

### CLI སྐམ་རྒྱུག་ (གདམ་ཁ།)

ཨེཆ་ཊི་ཊི་པི་མཇུག་བསྡུའི་ས་སྒོ་ཚུ་ ཕྱིར་བཏོན་མ་འབད་བའི་ཧེ་མ་ ཁྱོད་ཀྱིས་ ཀླད་ཀོར་གྱི་རྒྱབ་གཞི་འདི་ བཱན་ཌི་འབད་ཡོད་པའི་སི་ཨེལ་ཨའི་ སི་ཨེལ་ཨའི་ དང་གཅིག་ཁར་ཞིབ་དཔྱད་འབད་ཚུགས།

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

བརྡ་བཀོད་ཚུ་གིས་ དཔར་བསྐྲུན་ Norito JSON བཅུད་བསྡུས་དང་ ཅངཀ་-གསལ་སྡུད་ཡང་ན་ བཞུ་ཁུ་མ་མཐུནམ་ཚུ་ ངོས་ལེན་མ་འབད་བར་ Torii གློག་ཐག་ལས་ གདོང་ཁར་ CI དུ་པའི་ཞིབ་དཔྱད་ཀྱི་དོན་ལུ་ ཕན་ཐོགས་ཅན་བཟོཝ་ཨིན།

ཚར་གཅིག་ Torii འདི་ ཐད་རི་བ་རི་ཨིན་ ཁྱོད་ཀྱིས་ HTTP བརྒྱུད་དེ་ ཅ་རྙིང་གཅིག་མཚུངས་ཚུ་ ལོག་ཐོབ་ཚུགས།

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

མཐའ་མའི་གཉིས་ཆ་ར་ བཙུགས་ཡོད་པའི་ གསོག་འཇོག་འབད་མི་ལཱ་འབད་མི་ཚུ་གིས་ ཞབས་ཏོག་བྱིན་དོ་ཡོདཔ་ལས་ CLI དུ་པའི་བརྟག་དཔྱད་དང་ འཛུལ་སྒོ་གི་འཚོལ་ཞིབ་ཚུ་ མཉམ་མཐུན་སྦེ་སྡོདཔ་ཨིན།

## 2. པིན → ཕེཆ་སྐོར་རིམ་ཊིརིཔ་

༡ གསལ་སྟོན་ཅིག་བཟོ་བསྐྲུན་འབད། + པེ་ལོཌི་བཱན་ཌལ་ (དཔེར་ན་ `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` དང་གཅིག་ཁར་)།
2. གཞི་རྟེན་༦༤ ཨིན་ཀོ་ཌིང་དང་གཅིག་ཁར་ གསལ་སྟོན་འདི་ཕུལ་ནི།

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   ཞུ་བ་ནང་ JSON ནང་ `manifest_b64` དང་ `payload_b64` ཡོད་དགོ། མཐར་འཁྱོལ་ཅན་གྱི་ལན་འདེབས་འདི་གིས་ `manifest_id_hex` དང་ འབབ་ཁུངས་འདི་གིས་ བཞུ་བཅུགཔ་ཨིན།
3. པིན་འབད་ཡོད་པའི་གནས་སྡུད་འདི་ལེན།

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   གཞི་རྟེན་༦༤-ཌི་ཀོཌི་ `data_b64` ས་སྒོ་དང་ བདེན་དཔྱད་འབད་ བཱའིཊི་ངོ་མ་དང་མཐུན་སྒྲིག་འབད།

## 3. སླར་གསོ་འབད་ནི།

༡ གོང་དུ་བཤད་པ་ལྟར་ཉུང་མཐར་མངོན་པའི་མངོན་རྟགས་གཅིག་པིན།
2. Torii ལས་སྦྱོར་འདི་ལོག་འགོ་བཙུགས།(ཡང་ན་ མཐུད་མཚམས་ཧྲིལ་བུ་)།
༣ ཕེཆ་ཞུ་བ་འདི་ལོག་སྟེ་བཙུགས་དགོ། པེ་ལོཌ་འདི་ ད་ལྟོ་ཡང་ ལོག་ཐོབ་ཚུགསཔ་ཨིནམ་ལས་ སླར་ལོག་འབད་མི་ ཟས་བཅུད་འདི་གིས་ སྔོན་འགྲོའི་ལོག་འགོ་བཙུགས་གནས་གོང་དང་མཐུན་དགོཔ་ཨིན།
4. `GET /v1/sorafs/storage/state` གིས་ `bytes_used` འདི་ ལོག་འགོ་བཙུགས་པའི་ཤུལ་ལས་ གནས་ཏེ་ཡོད་པའི་ མངོན་གསལ་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།

## 4. ཆོས་ཚན་བཀག་ཆ་བརྟག་དཔྱད།

༡ གནས་སྐབས་ཅིག་ `torii.sorafs.storage.max_capacity_bytes` གནས་གོང་ཆུང་ཆུང་ལུ་མར་ཕབ་འབད་ཡོདཔ་ཨིན་ (དཔེར་ན་ གསལ་སྟོན་རྐྱང་པའི་ཚད་གཞི།)
2. Pin གཅིག་མངོན་སུམ་དང་། ཞུ་བ་འདི་ མཐར་འཁྱོལ་དགོ།
༣ ཚད་གཞི་འདྲ་མཚུངས་ཀྱི་ གསལ་སྟོན་གཉིས་པ་ཅིག་ བཏབ་ནིའི་དཔའ་བཅམ། Torii གིས་ HTTP `400` དང་ `storage capacity exceeded` ཡོད་པའི་འཛོལ་བའི་འཕྲིན་དོན་ཅིག་དང་གཅིག་ཁར་ ཞུ་བ་འདི་ ངོས་ལེན་མ་འབད་བར་བཞག་དགོ།
༤ མཇུག་བསྡུ་བའི་སྐབས་ སྤྱིར་བཏང་ལྕོགས་གྲུབ་ཚད་གཞི་འདི་ སླར་སྒྲིག་འབད།

## 5. POR དཔེ་ཚད་ཞིབ་འཇུག།

1. པིན་མངོན་དུ་བྱེད་པ།
2. པོ་ཨར་དཔེ་ཚད་ཅིག་ཞུ་བ་འབད།

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

༣ ལན་འདི་བདེན་བཤད་འབད་ ཞུ་བ་འབད་ཡོད་པའི་གྱངས་ཁ་དང་གཅིག་ཁར་ `samples` དང་ བདེན་བཤད་རེ་རེ་གིས་ གསོག་འཇོག་འབད་ཡོད་པའི་གསལ་སྟོན་རྩ་བ་ལུ་ བདེན་དཔྱད་འབདཝ་ཨིན།

## 6. རང་འགུལ།

- CI / ཐ་མག་བརྟག་དཔྱད་ཚུ་གིས་ དམིགས་གཏད་ཅན་གྱི་ཞིབ་དཔྱད་ཚུ་ ལོག་སྟེ་ལག་ལེན་འཐབ་ཚུགས།

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```དེ་ཡང་ `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, དང་ Torii, ཚུ་ཁྱབ་སྟེ་ཡོདཔ་ཨིན།
- ཌེཤ་བོརཌི་ཚུ་གིས་ བརྟག་ཞིབ་འབད་དགོ།
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` དང་ `torii_sorafs_storage_fetch_inflight`
  - པོ་ཨར་ མཐར་འཁྱོལ་/འཐུས་ཤོར་གྱི་ གྱངས་ཁ་ཚུ་ `/v1/sorafs/capacity/state` བརྒྱུད་དེ་ ཐོན་ཡོདཔ།
  - གཞིས་ཆགས་དཔར་བསྐྲུན་འབད་ནི་ དཔའ་བཅམ་མི་ཚུ་ `sorafs_node_deal_publish_total{result=success|failure}` བརྒྱུད་དེ་ དཔར་བསྐྲུན་འབདཝ་ཨིན།

འ་ནི་སྦྱོང་བརྡར་ཚུ་གི་ཤུལ་ལས་ བཙུགས་ཡོད་པའི་ གསོག་འཇོག་འབད་མི་ལཱ་འབད་མི་ཚུ་གིས་ གནས་སྡུད་ཚུ་ བཙུགས་ནི་དང་ ལོག་སྟེ་འགོ་བཙུགས་ནི་ དེ་ལས་ རིམ་སྒྲིག་འབད་ཡོད་པའི་ ཚད་རིམ་ཚུ་ལུ་ གུས་ཞབས་འབད་ནི་ དེ་ལས་ མཐུད་མཚམས་ལུ་ ལྕོགས་གྲུབ་ཁྱབ་བསྒྲགས་མ་འབད་བའི་ཧེ་མ་ གཏན་འབེབས་བཟོ་མི་ པོ་ཨར་གྱི་བདེན་ཁུངས་ཚུ་ བཟོ་བཏོན་འབད་ནི་ལུ་ ངེས་གཏན་བཟོཝ་ཨིན།