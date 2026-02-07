---
lang: am
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6113bf3d608167f409a9a20b80770bb3ba1d3c050e97492070ee2f9ac706d567
source_last_modified: "2026-01-05T09:28:11.916816+00:00"
translation_last_reviewed: 2026-02-07
id: transport
title: SoraNet transport overview
sidebar_label: Transport Overview
description: Handshake, salt rotation, and capability guidance for the SoraNet anonymity overlay.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

SoraNet የSoraFS ክልል ፈልጎዎችን፣ Norito RPC ዥረትን እና የወደፊት የNexus የመረጃ መስመሮችን የሚደግፍ ማንነትን መደበቅ ተደራቢ ነው። የትራንስፖርት መርሃ ግብሩ (የመንገድ ካርታ እቃዎች **SNNet-1**፣ **SNNet-1a** እና **SNNet-1b**) የሚወስን መጨባበጥ፣ድህረ-ኳንተም (PQ) የችሎታ ድርድር እና የጨው ሽክርክር እቅድ እያንዳንዱ ቅብብሎሽ፣ ደንበኛ እና መግቢያ በር ተመሳሳይ የደህንነት አቋም እንዲታይ ወስኗል።

## ግቦች እና የአውታረ መረብ ሞዴል

- በQUIC v1 ላይ ባለ ሶስት-ሆፕ ወረዳዎችን (መግቢያ → መካከለኛ → መውጫ) ይገንቡ ስለዚህ ተሳዳቢ እኩዮች በቀጥታ ወደ Torii አይደርሱም።
- የክፍለ ጊዜ ቁልፎችን ከTLS ግልባጭ ጋር ለማያያዝ በQUIC/TLS ላይ የ Noise XX *ድብልቅ* መጨባበጥ (Curve25519 + Kyber768) ንብር።
- PQ KEM/የፊርማ ድጋፍን፣ የማስተላለፊያ ሚናን እና የፕሮቶኮል ሥሪትን የሚያስተዋውቁ የችሎታ TLVዎችን ይፈልጋሉ። የወደፊት ማራዘሚያዎች ተዘርግተው እንዲቆዩ የማይታወቁ ዓይነቶችን ይቀቡ።
- የማውጫ ጩኸት የደንበኞቹን ማንነታቸው እንዳይገለጽ በየቀኑ የታወሩ የይዘት ጨዎችን እና የፒን ጠባቂ ማሰራጫዎችን ለ30 ቀናት ያሽከርክሩ።
- ሕዋሶችን በ1024B እንዲጠግኑ ያድርጉ፣ ፕላዲንግ/ዱሚ ህዋሶችን በመርፌ እና ወደ ውጭ የሚላኩ ዲሲሜትሪቲስቲክ ቴሌሜትሪ ስለዚህ የማውረድ ሙከራዎች በፍጥነት እንዲያዙ ያድርጉ።

## የመጨባበጥ ቧንቧ መስመር (SNNet-1a)

1. **QUIC/TLS ኤንቨሎፕ** - ደንበኞች በQUIC v1 ላይ ሪሌይ ይደውላሉ እና በአስተዳደር CA የተፈረመ በ Ed25519 የምስክር ወረቀቶች በመጠቀም TLS1.3 መጨባበጥ ያጠናቅቃሉ። የቲኤልኤስ ላኪ (`tls-exporter("soranet handshake", 64)`) የ Noise ንብርብርን ይዘራል ስለዚህ ግልባጮቹ የማይነጣጠሉ ናቸው።
2. ** ጫጫታ ኤክስኤክስ ዲቃላ** - የፕሮቶኮል ሕብረቁምፊ `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` ከመቅድም ጋር = TLS ላኪ። የመልእክት ፍሰት፡-

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH ውፅዓት እና ሁለቱም የ Kyber ኢንካፕሱሎች ወደ መጨረሻው ሲሜትሪክ ቁልፎች ይደባለቃሉ። የPQ ቁሳቁስ አለመደራደር መጨባበጥን ያስወገደው - ምንም ክላሲካል-ብቻ ውድቀት አይፈቀድም።

3. ** የእንቆቅልሽ ቲኬቶች እና ቶከኖች** - ሪሌሎች ከ`ClientHello` በፊት Argon2id የማረጋገጫ ትኬት ሊጠይቁ ይችላሉ። ትኬቶች የሃሽ Argon2 መፍትሄን የሚሸከሙ እና በመመሪያው ወሰን ውስጥ ጊዜያቸው የሚያልፍባቸው ረጅም ቅድመ ቅጥያ ክፈፎች ናቸው።

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   የML-DSA-44 ፊርማ ከአውጪው ከገባሪ ፖሊሲ እና የስረዛ ዝርዝር ጋር ሲወዳደር የመግቢያ ቶከኖች በ`SNTK` ማለፊያ እንቆቅልሾች ቅድመ ቅጥያ።

4. ** የችሎታ TLV ልውውጥ *** - የመጨረሻው የድምፅ ጭነት ከዚህ በታች የተገለጹትን የችሎታ TLVs ያጓጉዛል። ማንኛውም የግዴታ አቅም (PQ KEM/ፊርማ፣ ሚና ወይም ስሪት) ከጎደለ ወይም ከማውጫው ግቤት ጋር ካልተዛመደ ደንበኞች ግንኙነቱን ያቋርጣሉ።

5. **የጽሑፍ ግልባጭ ምዝግብ ማስታወሻ** - የማስተላለፊያ ደብተሮች መዝገብ የጽሑፍ ግልባጭ ሃሽ፣ TLS የጣት አሻራ እና የ TLV ይዘቶችን ዝቅ ለማድረግ መመርመሪያዎችን እና ተገዢ ቧንቧዎችን ለመመገብ።

## አቅም TLVs (SNNet-1c)

ችሎታዎች ቋሚ `typ/length/value` TLV ፖስታ እንደገና መጠቀም

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

ዛሬ የተገለጹ ዓይነቶች፡-

- `snnet.pqkem` - Kyber ደረጃ (ለአሁኑ ልቀት `kyber768`)።
- `snnet.pqsig` – PQ ፊርማ ስብስብ (`ml-dsa-44`)።
- `snnet.role` - የማስተላለፊያ ሚና (`entry`, `middle`, `exit`, I18NI0000021X).
- `snnet.version` - የፕሮቶኮል ሥሪት መለያ።
- `snnet.grease` - የወደፊት ቲኤልቪዎች መታገስን ለማረጋገጥ በተያዘው ክልል ውስጥ የዘፈቀደ መሙያ ግቤቶች።

ደንበኞች የሚፈለጉትን TLVዎች የተፈቀደላቸው ዝርዝር ይይዛሉ እና እነሱን የሚተው ወይም ዝቅ የሚያደርጉትን መጨባበጥ አይሳኩም። ማስተላለፎች ተመሳሳይ ስብስብ በማውጫቸው ማይክሮ ገለጻ ውስጥ ያትማሉ ስለዚህ ማረጋገጥ የሚወስነው።

## የጨው ሽክርክሪት እና የ CID ዓይነ ስውር (SNNet-1b)

- አስተዳደር የI18NI0000024X ሪኮርድን ከ`(epoch_id, salt, valid_after, valid_until)` እሴቶች ጋር ያትማል። ማስተላለፎች እና መግቢያዎች የተፈረመውን መርሐግብር ከማውጫ አታሚው ያገኙታል።
- ደንበኞች አዲሱን ጨው በI18NI0000026X ይተገብራሉ፣ የቀደመውን ጨው ለ12 ሰአታት የእፎይታ ጊዜ ያቆዩት እና የተዘገዩ ዝመናዎችን ለመቋቋም የ7-ኢፖክ ታሪክ ያቆያሉ።
- ቀኖናዊ ዓይነ ስውር መለያዎች ይጠቀማሉ፡-

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  ጌትዌይስ የታወረውን ቁልፍ በ`Sora-Req-Blinded-CID` ተቀብለው በ`Sora-Content-CID` አስተጋባ። የወረዳ/ጥያቄ ዓይነ ስውር (`CircuitBlindingKey::derive`) በ`iroha_crypto::soranet::blinding` ውስጥ ይጓዛሉ።
- ሪሌይ አንድ ዘመን ካጣ፣ መርሃ ግብሩን እስኪያወርድ ድረስ አዳዲስ ወረዳዎችን ያቆማል እና `SaltRecoveryEventV1` ያወጣል፣ ይህም የጥሪ ዳሽቦርዶች እንደ ፔጂንግ ሲግናል ይቆጥሩታል።

## የማውጫ መረጃ እና የጥበቃ ፖሊሲ

- ማይክሮ ዲስክሪፕተሮች የሪሌይ መታወቂያ (Ed25519 + ML-DSA-65)፣ PQ ቁልፎች፣ የችሎታ TLVs፣ የክልል መለያዎች፣ የጥበቃ ብቁነት እና በአሁኑ ጊዜ ማስታወቂያ የተደረገውን የጨው ዘመን ይይዛሉ።
- የደንበኞች ፒን ጠባቂ ለ 30 ቀናት ያዘጋጃል እና `guard_set` መሸጎጫዎች ከተፈረመበት የማውጫ ቅጽበታዊ ገጽ እይታ ጎን ይቆያሉ። የCLI እና የኤስዲኬ መጠቅለያዎች የመሸጎጫ አሻራውን ስላሳለፉ ግምገማዎችን ለመቀየር የታቀዱ ማስረጃዎች ማያያዝ ይችላሉ።

## ቴሌሜትሪ እና የታቀዱ የፍተሻ ዝርዝር

- ከምርት በፊት ወደ ውጭ ለመላክ መለኪያዎች;
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- የማንቂያ ገደቦች ከጨው ሽክርክሪት SOP SLO ማትሪክስ (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) ጋር አብረው ይኖራሉ እና አውታረ መረቡ ከመስፋፋቱ በፊት በአለርትማኔጀር ውስጥ መንጸባረቅ አለባቸው።
- ማንቂያዎች፡ > 5% የውድቀት መጠን ከ5ደቂቃ በላይ፣ የጨው መዘግየት > 15ደቂቃዎች፣ ወይም የችሎታ አለመመጣጠን በምርት ላይ ይስተዋላል።
- የታቀዱ ደረጃዎች;
  1. በድብልቅ የእጅ መጨባበጥ እና PQ ቁልል የነቃ የእንቅስቃሴ ቅብብል/የደንበኛ መስተጋብር ሙከራዎች።
  2. የጨው ሽክርክሪት SOP (`docs/source/soranet_salt_plan.md`) ይለማመዱ እና የመሰርሰሪያ ቅርሶችን ከለውጥ መዝገብ ጋር ያያይዙ።
  3. በማውጫው ውስጥ የችሎታ ድርድርን ያንቁ፣ ከዚያ ወደ የመግቢያ ቅብብሎሽ፣ መካከለኛ ቅብብሎሽ፣ መውጫዎች እና በመጨረሻም ደንበኞች ይልቀቁ።
  4. ለእያንዳንዱ ደረጃ የጥበቃ መሸጎጫ አሻራዎችን፣ የጨው መርሃ ግብሮችን እና የቴሌሜትሪ ዳሽቦርዶችን ይመዝግቡ። የማስረጃውን ጥቅል ከ I18NI0000040X ጋር ያያይዙ።

ይህንን የፍተሻ ዝርዝር በመከተል ኦፕሬተር፣ ደንበኛ እና ኤስዲኬ ቡድኖች በSNNet ፍኖተ ካርታ ውስጥ የተካተቱትን የቁርጥነት እና የኦዲት መስፈርቶች በሚያሟሉበት ጊዜ የ SoraNet ትራንስፖርትን በመቆለፊያ ደረጃ እንዲወስዱ ያስችላቸዋል።