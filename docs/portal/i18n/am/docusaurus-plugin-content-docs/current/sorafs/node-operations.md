---
id: node-operations
lang: am
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
መስተዋቶች `docs/source/sorafs/runbooks/sorafs_node_ops.md`. የ Sphinx ስብስብ ጡረታ እስኪወጣ ድረስ ሁለቱንም ስሪቶች በማመሳሰል ያቆዩዋቸው።
::

## አጠቃላይ እይታ

ይህ Runbook ኦፕሬተሮችን በTorii ውስጥ የተካተተ `sorafs-node` ስርጭቱን በማረጋገጥ ይራመዳል። እያንዳንዱ ክፍል በቀጥታ ወደ SF-3 ማቅረቢያዎች ያዘጋጃል፡- ፒን/የዙር ጉዞዎችን ማምጣት፣ ማገገምን እንደገና ማስጀመር፣ ኮታ አለመቀበል እና የPoR ናሙና።

## 1. ቅድመ ሁኔታዎች

- የማከማቻ ሰራተኛውን በ`torii.sorafs.storage` ውስጥ አንቃ፡

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

- የ Torii ሂደት የ `data_dir` የማንበብ/የመፃፍ መዳረሻ እንዳለው ያረጋግጡ።
- መስቀለኛ መንገዱ የሚጠበቀውን አቅም በ`GET /v1/sorafs/capacity/state` በኩል አንድ ጊዜ መግለጫ ከተመዘገበ ያረጋግጡ።
- ማለስለስ ሲነቃ ዳሽቦርዶች ጥሬውን እና ለስላሳውን የጂቢሆር/PoR ቆጣሪዎችን ከቦታ እሴቶች ጎን ለጎን ከጅት ነፃ የሆኑ አዝማሚያዎችን ለማጉላት ያጋልጣሉ።

### CLI ደረቅ ሩጫ (አማራጭ)

የኤችቲቲፒ የመጨረሻ ነጥቦችን ከማጋለጥዎ በፊት የማከማቻውን ጀርባ በተጠቀለለ CLI ይመልከቱ።【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

ትእዛዞቹ I18NT0000000X JSON ማጠቃለያዎችን ያትሙ እና የተቆራረጡ መገለጫዎችን እምቢ ይላሉ ወይም አለመዛመጃዎችን አይፍጩ፣ ይህም ለ CI ጭስ ፍተሻዎች ከTorii ሽቦ በፊት ጠቃሚ ያደርጋቸዋል።【crates/sorafs_node/tests/cli.rs#L1】

### የPoR ማረጋገጫ ልምምድ

ኦፕሬተሮች አሁን በአስተዳደር የተሰጡ የPoR ቅርሶችን ወደ Torii ከመጫንዎ በፊት እንደገና ማጫወት ይችላሉ። CLI ተመሳሳዩን I18NI0000023X የማስገቢያ ዱካውን እንደገና ይጠቀማል፣ ስለዚህ የሀገር ውስጥ ሂደቶች የኤችቲቲፒ ኤፒአይ የሚመልሳቸውን ትክክለኛ የማረጋገጫ ስህተቶችን ያሳያል።

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

ትዕዛዙ የJSON ማጠቃለያ (ገላጭ ዳይጀስት፣ የአቅራቢ መታወቂያ፣ የማረጋገጫ መፍቻ፣ የናሙና ብዛት፣ የአማራጭ የፍርድ ውጤት) ያወጣል። የተከማቸ አንጸባራቂ ከተፈታታኝ ሁኔታ ጋር የሚዛመድ መሆኑን ለማረጋገጥ `--manifest-id=<hex>` ያቅርቡ እና ለኦዲት ማስረጃዎች ማጠቃለያውን ከመጀመሪያዎቹ ቅርሶች ጋር በማህደር ለማስቀመጥ ሲፈልጉ `--json-out=<path>` ያቅርቡ። `--verdict` ን ጨምሮ የኤችቲቲፒ ኤፒአይ ከመደወልዎ በፊት ሙሉውን ፈተና →ማስረጃ → ከመስመር ውጭ ብይን እንዲለማመዱ ያስችልዎታል።

አንዴ Torii ቀጥታ ስርጭት ከሆነ ተመሳሳይ ቅርሶችን በኤችቲቲፒ ማግኘት ይችላሉ።

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

ሁለቱም የመጨረሻ ነጥቦች የሚቀርቡት በተሰቀለው የማከማቻ ሰራተኛ ነው፣ ስለዚህ የCLI የጭስ ሙከራዎች እና የጌትዌይ ፍተሻዎች ሳይመሳሰሉ ይቀራሉ።【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. ፒን → የክብ ጉዞን አምጣ

1. የማኒፌክት + የመጫኛ ጥቅል (ለምሳሌ በ`iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`) ያዘጋጁ።
2. አንጸባራቂውን በbase64 ኢንኮዲንግ ያስገቡ፡-

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON ጥያቄው `manifest_b64` እና I18NI0000029X መያዝ አለበት። የተሳካ ምላሽ I18NI0000030X እና የተጫነውን ጭነት ይመልሳል።
3. የተሰካውን ውሂብ ያውጡ፡

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-የ`data_b64` መስኩን መፍታት እና ከዋናው ባይት ጋር መዛመዱን ያረጋግጡ።

## 3. የመልሶ ማግኛ ቁፋሮውን እንደገና ያስጀምሩ

1. ከላይ እንደተገለጸው ቢያንስ አንድ አንጸባራቂ ይሰኩት።
2. የ Torii ሂደቱን (ወይም ሙሉውን መስቀለኛ መንገድ) እንደገና ያስጀምሩ.
3. የማምጣት ጥያቄውን እንደገና ያስገቡ። የተጫነው ጭነት አሁንም ተመልሶ ሊወጣ የሚችል መሆን አለበት እና የተመለሰው የምግብ መፍጫ ከቅድመ-ዳግም ማስጀመር ዋጋ ጋር መዛመድ አለበት።
4. `bytes_used` ለማረጋገጥ I18NI0000032Xን መርምር ከዳግም ማስነሳቱ በኋላ የቆዩትን መገለጫዎች የሚያንፀባርቅ ነው።

## 4. የኮታ ውድቅ ሙከራ

1. ለጊዜው I18NI0000034X ወደ ትንሽ እሴት ዝቅ አድርግ (ለምሳሌ የአንድ አንጸባራቂ መጠን)።
2. ፒን አንድ አንጸባራቂ; ጥያቄው ሊሳካለት ይገባል.
3. ተመሳሳይ መጠን ያለው ሁለተኛ አንጸባራቂ ለመሰካት ይሞክሩ። Torii ጥያቄውን በ HTTP I18NI0000035X እና `storage capacity exceeded` የያዘ የስህተት መልእክት ውድቅ ማድረግ አለበት።
4. ሲጨርሱ መደበኛውን የአቅም ገደብ ይመልሱ.

## 5. ማቆየት / ጂሲ ምርመራ (ተነባቢ-ብቻ)

1. በማከማቻ ማውጫው ላይ የአካባቢ ማቆያ ቅኝትን ያሂዱ፡

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. ጊዜው ያለፈባቸው አንጸባራቂዎችን ብቻ መርምር (በደረቅ አሂድ ብቻ፣ ምንም ስረዛ የለም)።

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. በአስተናጋጆች ወይም በአጋጣሚዎች ላይ ሪፖርቶችን ሲያወዳድሩ የግምገማ መስኮቱን ለመሰካት `--now` ወይም I18NI0000038X ይጠቀሙ።

GC CLI ሆን ተብሎ ተነባቢ ብቻ ነው። የማቆያ ቀነ-ገደቦችን እና ጊዜው ያለፈበት-የኦዲት ዱካዎች ዝርዝር መረጃን ለመያዝ ይጠቀሙበት። በምርት ውስጥ መረጃን በእጅ አያስወግዱ.

## 6. PoR ናሙና ምርመራ

1. መግለጫ ሰካ።
2. የPoR ናሙና ጠይቅ፡-

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. ምላሹ ከተጠየቀው ቆጠራ ጋር `samples` መያዙን ያረጋግጡ እና እያንዳንዱ ማስረጃ ከተከማቸ አንጸባራቂ ስር ይፀድቃል።

## 7. አውቶሜሽን መንጠቆዎች

- CI/ጭስ ሙከራዎች የታለሙትን ቼኮች እንደገና መጠቀም ይችላሉ፡-

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  ይህም `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, እና I18NI0000043X ይሸፍናል.
- ዳሽቦርዶች መከታተል አለባቸው:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` እና `torii_sorafs_storage_fetch_inflight`
  - የPoR ስኬት/የሽንፈት ቆጣሪዎች በ`/v1/sorafs/capacity/state` በኩል ብቅ አሉ።
  - የመቋቋሚያ ሙከራዎችን በ`sorafs_node_deal_publish_total{result=success|failure}` በኩል ያትማል

እነዚህን ልምምዶች መከተል የመስቀለኛ መንገዱ አቅም ለሰፊው አውታረመረብ ከማስተዋወቁ በፊት የተካተተ የማከማቻ ሰራተኛ መረጃን ወደ ውስጥ ማስገባት፣ ዳግም ሲጀመር መትረፍ፣ የተዋቀሩ ኮታዎችን ማክበር እና ቆራጥ የPoR ማረጋገጫዎችን ማመንጨት መቻሉን ያረጋግጣል።