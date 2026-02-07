---
lang: am
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# የታይካይ መልህቅ የዘር መስመር ፓኬት አብነት (SN13-ሲ)

የመንገድ ካርታ ንጥል ** SN13-ሲ - መግለጫዎች እና የሶራኤንኤስ መልህቆች ** እያንዳንዱን ስም ይጠይቃል
የመወሰኛ ማስረጃ ጥቅል ለመላክ ማሽከርከር። ይህንን አብነት ወደ እርስዎ ይቅዱ
የቅርጻ ቅርጽ ማውጫን መልቀቅ (ለምሳሌ
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) እና ተካ
ፓኬጁን ወደ አስተዳደር ከማቅረቡ በፊት ቦታ ያዢዎቹ.

## 1. ሜታዳታ

| መስክ | ዋጋ |
|-------|------|
| የክስተት መታወቂያ | `<taikai.event.launch-2026-07-10>` |
| ዥረት / አተረጓጎም | `<main-stage>` |
| ተለዋጭ ስም ቦታ / ስም | `<sora / docs>` |
| የማስረጃ ማውጫ | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| ኦፕሬተር ግንኙነት | `<name + email>` |
| GAR / RPT ትኬት | `<governance ticket or GAR digest>` |

## ጥቅል ረዳት (አማራጭ)

የ spool artefacts ይቅዱ እና ከዚህ በፊት JSON (በአማራጭ የተፈረመ) ማጠቃለያ ይላኩ።
የተቀሩትን ክፍሎች መሙላት;

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

ረዳቱ `taikai-anchor-request-*`፣ `taikai-trm-state-*`፣
`taikai-lineage-*`፣ ፖስታዎች እና ሴንቴሎች ከታይካይ ስፑል ማውጫ ወጥተዋል
(`config.da_ingest.manifest_store_dir/taikai`) ስለዚህ የማስረጃ ማህደሩ አስቀድሞ
ከዚህ በታች የተጠቀሱትን ትክክለኛ ፋይሎች ይዟል.

## 2. የዘር ሐረግ እና ፍንጭ

ሁለቱንም በዲስክ ላይ ያለውን የዘር መዝገብ እና JSON I18NT0000003X የፃፈውን ፍንጭ ያያይዙ
መስኮት. እነዚህ በቀጥታ የሚመጡት
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` እና
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefact | ፋይል | SHA-256 | ማስታወሻ |
|---------|-----|-------|
| የዘር መዝገብ | `taikai-trm-state-docs.json` | `<sha256>` | የቀደመውን አንጸባራቂ መፍጨት/መስኮት ያረጋግጣል። |
| የዘር ፍንጭ | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | ወደ SoraNS መልህቅ ከመጫኑ በፊት ተይዟል። |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. የመልህቅ ጭነት ቀረጻ

Torii ወደ መልህቅ አገልግሎት ያደረሰውን የPOST ጭነት ይመዝግቡ። ክፍያው
`envelope_base64`፣ `ssm_base64`፣ `trm_base64`፣ እና የውስጥ መስመርን ያካትታል።
`lineage_hint` ነገር; ኦዲቶች የነበረውን ፍንጭ ለማረጋገጥ በዚህ ቀረጻ ላይ ይመረኮዛሉ
ወደ SoraNS ተልኳል። Torii አሁን ይህንን JSON በራስ ሰር ይጽፋል
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
በታይካይ ስፑል ማውጫ (`config.da_ingest.manifest_store_dir/taikai/`) ውስጥ፣ እ.ኤ.አ
ኦፕሬተሮች የኤችቲቲፒ ምዝግብ ማስታወሻዎችን ከመቧጨር ይልቅ በቀጥታ መቅዳት ይችላሉ።

| Artefact | ፋይል | SHA-256 | ማስታወሻ |
|---------|-----|-------|
| መልህቅ POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | ጥሬ ጥያቄ ከI18NI0000035X (ታይካይ ስፑል) ተቀድቷል። |

## 4. የምግብ መፈጨት እውቅናን ማሳየት

| መስክ | ዋጋ |
|-------|------|
| አዲስ አንጸባራቂ መፍጨት | `<hex digest>` |
| የቀድሞ አንጸባራቂ መፍጨት (ከፍንጭ) | `<hex digest>` |
| የመስኮት መጀመሪያ / መጨረሻ | `<start seq> / <end seq>` |
| የመቀበል ጊዜ ማህተም | `<ISO8601>` |

ገምጋሚዎች ማረጋገጥ እንዲችሉ ከላይ የተመዘገቡትን የሂሳብ ደብተር/ፍንጭ ሃሽ ይመልከቱ
የተተካው መስኮት.

## 5. መለኪያዎች / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` ቅጽበታዊ እይታ: I18NI0000042X
- `/status taikai_alias_rotations` መጣያ (በተለዋጭ ስም)፡ `<file path + hash>`

ቆጣሪውን የሚያሳየውን የPrometheus/Grafana ኤክስፖርት ወይም I18NI0000045X ውፅዓት ያቅርቡ
ጭማሪ እና የ I18NI0000046X አደራደር ለዚህ ተለዋጭ ስም።

## 6. ለማስረጃዎች ማውጫ ይገለጣል

የማስረጃ ዝርዝሩን የሚወስን መግለጫ ማፍለቅ (ስፑል ፋይሎች፣
ክፍያ ቀረጻ፣ ሜትሪክስ ቅጽበታዊ ገጽ እይታዎች) ስለዚህ አስተዳደር ያለእያንዳንዱን ሃሽ ማረጋገጥ ይችላል።
ማህደሩን በማንሳት ላይ.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefact | ፋይል | SHA-256 | ማስታወሻ |
|---------|-----|-------|
| ማስረጃ አንጸባራቂ | `manifest.json` | `<sha256>` | ይህንን ከአስተዳደር ፓኬት /ጋር ጋር አያይዘው. |

## 7. የማረጋገጫ ዝርዝር

- [ ] የዘር መዝገብ ተቀድቷል + hashed።
- [ ] የዘር ፍንጭ ተቀድቷል + ሃሽ።
- [ ] መልህቅ POST ክፍያ ተይዟል እና ሃሽድ።
- [ ] ተሞልቷል የምግብ መፍጫ ሠንጠረዥ።
- [ ] ሜትሪክስ ቅጽበተ-ፎቶዎች ወደ ውጭ ተልከዋል (`taikai_trm_alias_rotations_total`፣ `/status`)።
- [ ] አንጸባራቂ የመነጨው በ`scripts/repo_evidence_manifest.py` ነው።
- [ ] ወደ አስተዳደር የተሰቀለ ፓኬት ከ hashes + የእውቂያ መረጃ ጋር።

ይህን አብነት ለእያንዳንዱ ተለዋጭ መጠሪያ ማቆየት የሶራንኤስ አስተዳደርን ይጠብቃል።
ሊባዛ የሚችል ጥቅል እና የዘር ፍንጮችን በቀጥታ ከGAR/RPT ማስረጃ ጋር ያገናኛል።