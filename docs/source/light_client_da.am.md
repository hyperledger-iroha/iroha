---
lang: am
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ቀላል የደንበኛ ውሂብ ተገኝነት ናሙና

የብርሃን ደንበኛ ናሙና ኤፒአይ የተረጋገጡ ኦፕሬተሮችን እንዲያነሱ ያስችላቸዋል
በበረራ ውስጥ ላለ ማገጃ በመርክል የተረጋገጠ የ RBC ቁራጭ ናሙናዎች። ቀላል ደንበኞች
የዘፈቀደ የናሙና ጥያቄዎችን ማቅረብ ይችላል ፣ የተመለሱትን ማስረጃዎች በ
ቸንክ ሩት አስተዋውቋል፣ እና ያለሱ ውሂብ እንደሚገኝ እምነት ገንቡ
ሙሉውን ጭነት በማምጣት ላይ።

## የመጨረሻ ነጥብ

```
POST /v1/sumeragi/rbc/sample
```

የመጨረሻው ነጥብ ከተዋቀረው አንዱን የሚዛመድ `X-API-Token` ራስጌ ያስፈልገዋል
Torii ኤፒአይ ማስመሰያዎች። ጥያቄዎች በተጨማሪ በታሪፍ የተገደቡ እና ለዕለታዊ ተገዢ ናቸው።
በእያንዳንዱ ደዋይ ባይት በጀት; ከሁለቱም በላይ HTTP 429 ይመልሳል።

### የጥያቄ አካል

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` - ዒላማ እገዳ ሃሽ በሄክስ።
* `height`፣ `view` - ለ RBC ክፍለ ጊዜ ቱፕል መለየት።
* `count` - የሚፈለጉት የናሙናዎች ብዛት (ነባሪ ወደ 1 ፣ በማዋቀር የተከለለ)።
* `seed` - አማራጭ የሚወስን RNG ዘር እንደገና ሊባዛ ለሚችል ናሙና።

### ምላሽ ሰጪ አካል

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

እያንዳንዱ የናሙና ግቤት የ chunk ኢንዴክስ፣ ሎድ ባይት (ሄክስ)፣ SHA-256 ቅጠል ይዟል
መፈጨት፣ እና የመርክል ማካተት ማረጋገጫ (ከአማራጭ ወንድሞች እና እህቶች ጋር እንደ ሄክስ ኮድ
ሕብረቁምፊዎች). ደንበኞች የ`chunk_root` መስክን በመጠቀም ማረጋገጫዎችን ማረጋገጥ ይችላሉ።

## ገደቦች እና በጀት

* ** ከፍተኛ ናሙናዎች በጥያቄ *** - በ `torii.rbc_sampling.max_samples_per_request` በኩል ሊዋቀር ይችላል።
* ** ከፍተኛ ባይት በጥያቄ *** - `torii.rbc_sampling.max_bytes_per_request` በመጠቀም ተፈጻሚ ነው።
** ዕለታዊ ባይት በጀት** - በአንድ ደዋይ በ`torii.rbc_sampling.daily_byte_budget` በኩል ክትትል የሚደረግበት።
** ተመን መገደብ ** - የተወሰነ ማስመሰያ ባልዲ (`torii.rbc_sampling.rate_per_minute`) በመጠቀም ተፈጻሚ ነው።

ከማንኛውም ገደብ ያለፈ ጥያቄዎች HTTP 429 (የአቅም ገደብ) መመለስ። መቼ ቁርጥራጭ
መደብር አይገኝም ወይም ክፍለ-ጊዜው የሚጫነው ባይት የመጨረሻው ነጥብ ይጎድላል
HTTP 404 ይመልሳል።

## የኤስዲኬ ውህደት

### ጃቫስክሪፕት

`@iroha/iroha-js` የ`ToriiClient.sampleRbcChunks` ረዳትን ያጋልጣል ስለዚህ መረጃ
የመገኘት አረጋጋጮች የራሳቸውን ሣያሽከረክሩ የመጨረሻውን ነጥብ መጥራት ይችላሉ።
አመክንዮ ረዳቱ የሄክስ ጭነቶችን ያረጋግጣል፣ ኢንቲጀሮችን መደበኛ ያደርጋል እና ይመለሳል
ከላይ ያለውን የምላሽ ንድፍ የሚያንፀባርቁ የተተየቡ ነገሮች፡-

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

JS-04 ን በማገዝ አገልጋዩ የተሳሳተ መረጃ ሲመልስ ረዳቱ ይጥላል
ሙከራዎች ከሩስት እና ፓይዘን ኤስዲኬዎች ጎን ለጎን መመለሻዎችን ይገነዘባሉ። ዝገት
(`iroha_client::ToriiClient::sample_rbc_chunks`) እና Python
(`IrohaToriiClient.sample_rbc_chunks`) የመርከብ ተመጣጣኝ ረዳቶች; የትኛውንም ተጠቀም
ከእርስዎ የናሙና መታጠቂያ ጋር ይዛመዳል።