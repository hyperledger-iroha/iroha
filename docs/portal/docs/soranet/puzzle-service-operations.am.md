---
lang: am
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0581539af03125aca3ed8009387b4552361d982d8cdc80f6a8ae5bbe3e2f271f
source_last_modified: "2026-01-05T09:28:11.915872+00:00"
translation_last_reviewed: 2026-02-07
id: puzzle-service-operations
title: Puzzle Service Operations Guide
sidebar_label: Puzzle Service Ops
description: Operating the `soranet-puzzle-service` daemon for Argon2/ML-DSA admission tickets.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# የእንቆቅልሽ አገልግሎት ኦፕሬሽን መመሪያ

የ`soranet-puzzle-service` ዴሞን (`tools/soranet-puzzle-service/`) ጉዳዮች
የማስተላለፊያውን I18NI0000008X ፖሊሲ የሚያንፀባርቁ በአርጎን2 የሚደገፉ የመግቢያ ትኬቶች
እና፣ ሲዋቀር፣ ደላሎች ML-DSA መግቢያ ቶከኖች የጠርዝ ማስተላለፊያዎችን ወክለው።
አምስት የኤችቲቲፒ መጨረሻ ነጥቦችን ያጋልጣል፡

- `GET /healthz` - የቀጥታነት ምርመራ።
- `GET /v2/puzzle/config` - የተጎተቱትን ውጤታማ የPoW/እንቆቅልሽ መለኪያዎች ይመልሳል
  ከሪሌይ JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - የአርጎን2 ቲኬት ደቂቃዎች; አማራጭ JSON አካል
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  አጭር TTL ይጠይቃል (በመመሪያው መስኮት ላይ ተጣብቆ)፣ ትኬቱን ከ ሀ
  ግልባጭ ሃሽ፣ እና በቅብብሎሽ የተፈረመ ትኬት + የፊርማ አሻራ ይመልሳል
  የመፈረሚያ ቁልፎች ሲዋቀሩ.
- `GET /v2/token/config` - `pow.token.enabled = true` ገባሪውን ሲመልስ
  የማስመሰያ መመሪያ (የአከፋፋይ የጣት አሻራ፣ ቲቲኤል/የሰዓት-ስኬው ወሰኖች፣ የመተላለፊያ መታወቂያ፣
  እና የተዋሃደ የመሻሪያ ስብስብ).
- `POST /v2/token/mint` – የML-DSA መግቢያ ማስመሰያ ከቀረበው ጋር የተያያዘ ነው።
  ሃሽ ከቆመበት ቀጥል; የጥያቄው አካል `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }` ይቀበላል።

በአገልግሎቱ የሚመረቱ ትኬቶች በ ውስጥ ተረጋግጠዋል
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
የውህደት ሙከራ፣ እሱም ደግሞ በቮልሜትሪክ ዶኤስ ወቅት የማስተላለፊያ ስሮትሎችን የሚለማመዱ
ሁኔታዎች።【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## ማስመሰያ መስጠትን በማዋቀር ላይ

የማስተላለፊያ JSON መስኮችን በ `pow.token.*` ስር ያዘጋጁ (ተመልከት
`tools/soranet-relay/deploy/config/relay.entry.json` ለምሳሌ) ለማንቃት
ML-DSA ማስመሰያዎች ቢያንስ ሰጭውን ይፋዊ ቁልፍ እና አማራጭ ያቅርቡ
የስረዛ ዝርዝር፡-

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

የእንቆቅልሽ አገልግሎት እነዚህን እሴቶች እንደገና ይጠቀማል እና Noritoን በራስ-ሰር እንደገና ይጭናል
JSON የመሻሪያ ፋይል በሂደት ጊዜ። `soranet-admission-token` CLI ይጠቀሙ
(`cargo run -p soranet-relay --bin soranet_admission_token`) ለመፈተሽ እና ለመፈተሽ
ቶከኖች ከመስመር ውጭ፣ የI18NI0000024X ግቤቶችን በስረዛ ፋይሉ ላይ ጨምሩ እና ኦዲት ያድርጉ።
ዝማኔዎችን ወደ ምርት ከመግፋቱ በፊት ያሉ ምስክርነቶች።

የእንቆቅልሽ አገልግሎት ሰጪውን ሚስጥራዊ ቁልፍ በCLI ባንዲራዎች በኩል ያስተላልፉ፡-

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` ምስጢሩ ከባንድ ውጪ በሚተዳደርበት ጊዜም ይገኛል።
የመሳሪያ ቧንቧ መስመር. የስረዛ ፋይል ጠባቂው `/v2/token/config` ን ያቆያል;
መዘግየትን ለማስወገድ በI18NI0000027X ትዕዛዝ ማሻሻያዎችን ያስተባብሩ
የመሻር ሁኔታ.

ML-DSA-44 ይፋዊ ለማስተዋወቅ `pow.signed_ticket_public_key_hex` በሬሌይ JSON አቀናብር
የተፈረመ የፖው ቲኬቶችን ለማረጋገጥ የሚያገለግል ቁልፍ; `/v2/puzzle/config` ቁልፉን እና BLAKE3 ያስተጋባል
የጣት አሻራ (`signed_ticket_public_key_fingerprint_hex`) ደንበኞች አረጋጋጩን ይሰኩት።
የተፈረሙ ቲኬቶች ከሪሌይ መታወቂያው እና ከጽሑፍ ግልባጭ ማያያዣዎች ጋር የተረጋገጡ ናቸው እና ተመሳሳይ ይጋራሉ።
የስረዛ መደብር; ጥሬ 74-ባይት PoW ቲኬቶች የተፈረመበት የቲኬት አረጋጋጭ ዋጋ ያለው ሆኖ ይቆያል
የተዋቀረ። የፈራሚውን ሚስጥር በ I18NI0000031X ወይም
`--signed-ticket-secret-path` የእንቆቅልሽ አገልግሎት ሲጀመር; ጅምር ያልተዛመደውን ውድቅ ያደርጋል
ሚስጥሩ በ`pow.signed_ticket_public_key_hex` ላይ ካልተረጋገጠ keypairs።
`POST /v2/puzzle/mint` `"signed": true` (እና አማራጭ I18NI0000036X) ይቀበላል
ከጥሬ ትኬት ባይት ጎን Norito የተፈረመ ትኬት ይመለሱ። ምላሾች ያካትታሉ
`signed_ticket_b64` እና `signed_ticket_fingerprint_hex` የጣት አሻራዎችን ለመከታተል ለማገዝ።
የፈራሚው ሚስጥር ካልተዋቀረ ከ`signed = true` ጋር ያሉ ጥያቄዎች ውድቅ ይደረጋሉ።

## ቁልፍ የማዞሪያ መጫወቻ መጽሐፍ

1. ** አዲሱን ገላጭ ቃል ይሰብስቡ።** አስተዳደር ቅብብሎሹን ያትማል
   ገላጭ በማውጫው ቅርቅብ ውስጥ መፈጸም. የሄክስ ሕብረቁምፊውን ወደ ውስጥ ይቅዱ
   `handshake.descriptor_commit_hex` በሪሌይ JSON ውቅር ውስጥ ተጋርቷል።
   ከእንቆቅልሽ አገልግሎት ጋር.
2. **የእንቆቅልሽ ፖሊሲ ወሰኖችን ይገምግሙ።** የተዘመነውን ያረጋግጡ
   `pow.puzzle.{memory_kib,time_cost,lanes}` እሴቶች ከልቀት ጋር ይጣጣማሉ
   እቅድ. ኦፕሬተሮች የአርጎን2 ውቅርን የሚወስን መሆን አለባቸው
   ማስተላለፊያዎች (ቢያንስ 4MiB ማህደረ ትውስታ፣ 1≤ሌኖች≤16)።
3. ** ድጋሚ ማስጀመርን ደረጃ ያድርጉት።** አንዴ አስተዳደር ስር ያለውን ክፍል ወይም መያዣ እንደገና ይጫኑ
   የማዞሪያ መቁረጥን ያስታውቃል. አገልግሎቱ ትኩስ-ዳግም መጫን ድጋፍ የለውም; ሀ
   አዲሱን ገላጭ ቃል ለማንሳት እንደገና መጀመር ያስፈልጋል።
4. ** አረጋግጥ።** ትኬቱን በI18NI0000042X አውጥተህ አረጋግጥ
   ተመልሷል `difficulty` እና I18NI0000044X ከአዲሱ ፖሊሲ ጋር ይዛመዳሉ። የዘፈቀደ ዘገባ
   (`docs/source/soranet/reports/pow_resilience.md`) የሚጠበቀውን መዘግየት ይይዛል
   ለማጣቀሻ ገደቦች. ማስመሰያዎች ሲነቁ፣ `/v2/token/config` ወደ ላይ ያውጡ
   የማስታወቂያ ሰጭው የጣት አሻራ እና የስረዛ ቆጠራ ከዚህ ጋር መዛመዱን ያረጋግጡ
   የሚጠበቁ እሴቶች.

## የአደጋ ጊዜን አሰናክል

1. I18NI0000047X በተጋራ ቅብብል ውቅር ውስጥ አዘጋጅ። አቆይ
   `pow.required = true` ከሆነ hashcash የመመለሻ ትኬቶች የግዴታ መሆን አለባቸው።
2. የቆዩ ገላጮችን ላለመቀበል የ`pow.emergency` ግቤቶችን ያስገድዱ።
   የአርጎን2 በር ከመስመር ውጭ ነው።
3. ለውጡን ተግባራዊ ለማድረግ ሁለቱንም የማስተላለፊያ እና የእንቆቅልሽ አገልግሎትን እንደገና ያስጀምሩ።
4. ችግሩ ወደ ታች መውረድን ለማረጋገጥ `soranet_handshake_pow_difficulty`ን ተቆጣጠር
   የሚጠበቀው የሃሽካሽ ዋጋ፣ እና የ`/v2/puzzle/config` ሪፖርቶችን ያረጋግጡ
   `puzzle = null`.

## ክትትል እና ማስጠንቀቂያ

- ** መዘግየት SLO:** `soranet_handshake_latency_seconds` ይከታተሉ እና P95 ን ያስቀምጡ
  ከ 300ms በታች. የሶክ ሙከራ ማካካሻዎች ለጠባቂ የመለኪያ መረጃ ይሰጣሉ
  ስሮትልስ።【docs/source/soranet/reports/pow_resilience.md:1】
- ** የኮታ ግፊት:** `soranet_guard_capacity_report.py` ከቅብብል መለኪያዎች ጋር ይጠቀሙ
  `pow.quotas` cooldowns (`soranet_abuse_remote_cooldowns`፣
  `soranet_handshake_throttled_remote_quota_total`)።【docs/source/soranet/relay_audit_pipeline.md:68】
- ** የእንቆቅልሽ አሰላለፍ፡** `soranet_handshake_pow_difficulty` መመሳሰል አለበት።
  ችግር በ I18NI0000059X ተመልሷል። ልዩነት የቆየ ቅብብሎሽ ያሳያል
  config ወይም ያልተሳካ ዳግም ማስጀመር.
- ** ማስመሰያ ዝግጁነት፡** `/v2/token/config` ወደ `enabled = false` ከወረደ ማንቂያ
  ሳይታሰብ ወይም I18NI0000062X የቆዩ የጊዜ ማህተሞችን ከዘገበ። ኦፕሬተሮች
  ማስመሰያ በሚሆንበት ጊዜ የI18NT0000002X መሻሪያ ፋይልን በCLI በኩል ማሽከርከር አለበት።
  ይህ የመጨረሻ ነጥብ ትክክለኛ እንዲሆን ጡረታ ወጥቷል።
- **የአገልግሎት ጤና፡** `/healthz` በተለመደው የአኗኗር ዘይቤ እና ማንቂያ
  `/v2/puzzle/mint` HTTP 500 ምላሾችን ከመለሰ (የአርጎን2 መለኪያን ያመለክታል
  አለመመጣጠን ወይም RNG ውድቀቶች)። የማስመሰያ ስራ ስህተቶች በ HTTP 4xx/5xx በኩል ይታያሉ
  በ `/v2/token/mint` ላይ ምላሾች; ተደጋጋሚ ውድቀቶችን እንደ የገጽታ ሁኔታ ይያዙ።

## ተገዢነት እና ኦዲት ምዝግብ ማስታወሻ

የማስተላለፊያ ምክንያቶችን የሚያካትቱ የተዋቀሩ I18NI0000066X ክስተቶችን ያስወጣሉ።
የማቀዝቀዝ ቆይታዎች. በ ውስጥ የተገለፀውን የተጣጣመ የቧንቧ መስመር ያረጋግጡ
`docs/source/soranet/relay_audit_pipeline.md` እነዚህን ምዝግብ ማስታወሻዎች ያስገባል ስለዚህ እንቆቅልሽ ነው።
የፖሊሲ ለውጦች ኦዲት መደረግ አለባቸው። የእንቆቅልሽ በር ሲነቃ ማህደሩን ያስቀምጡ
የታቀዱ የቲኬት ናሙናዎች እና የNorito ውቅር ቅጽበታዊ ገጽ እይታ ከታቀዱ ጋር
ለወደፊት ኦዲት ቲኬት. የመግቢያ ቶከኖች ከጥገና መስኮቶች ቀድመው ተቀምጠዋል
በ `token_id_hex` እሴቶቻቸው መከታተል እና በ ውስጥ ማስገባት አለባቸው
የመሻሪያ ፋይል አንዴ ጊዜው ካለፈበት ወይም ከተሻሩ።