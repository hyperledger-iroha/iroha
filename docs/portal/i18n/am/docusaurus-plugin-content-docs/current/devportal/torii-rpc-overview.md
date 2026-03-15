---
lang: am
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Overview
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC አጠቃላይ እይታ

Norito-RPC ለTorii APIs ሁለትዮሽ መጓጓዣ ነው። ተመሳሳዩን የኤችቲቲፒ ዱካዎች እንደገና ይጠቀማል
እንደ `/v2/pipeline` ነገር ግን በ Norito የተቀረጹ የክፈፍ ጭነቶችን ይለዋወጣል ይህም ንድፍ ያካተቱ
hashes እና checksums. ቆራጥ፣ የተረጋገጡ ምላሾች ወይም ሲፈልጉ ይጠቀሙበት
የቧንቧ መስመር JSON ምላሾች ማነቆ ሲሆኑ.

## ለምን መቀየር?
- በCRC64 እና በ schema hashes መወሰኛ ፍሬም የመፍታት ስህተቶችን ይቀንሳል።
- የተጋሩ Norito ረዳቶች በኤስዲኬዎች ውስጥ ያሉትን የውሂብ-ሞዴል ዓይነቶችን እንደገና እንድትጠቀሙ ያስችሉዎታል።
- Torii Norito ክፍለ ጊዜዎችን በቴሌሜትሪ መለያ ሰጥቷል፣ ስለዚህ ኦፕሬተሮች መከታተል ይችላሉ።
ከቀረቡት ዳሽቦርዶች ጋር ጉዲፈቻ.

#ጥያቄ ማቅረብ

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v2/transactions/submit
```

1. ክፍያዎን በNorito ኮድ (`iroha_client`፣ ኤስዲኬ አጋዥዎች ወይም
   `norito::to_bytes`)።
2. ጥያቄውን በI18NI0000027X ይላኩ።
3. `Accept: application/x-norito` በመጠቀም የI18NT0000006X ምላሽ ይጠይቁ።
4. የሚዛመደውን የኤስዲኬ ረዳት በመጠቀም ምላሹን ይግለጹ።

ኤስዲኬ-ተኮር መመሪያ፡-
- ** ዝገት ***: I18NI0000029X ሲያቀናብሩ Norito በቀጥታ ይደራደራል
  የ `Accept` ራስጌ.
- ** ፓይቶን ***: `NoritoRpcClient` ከ `iroha_python.norito_rpc` ይጠቀሙ።
- **አንድሮይድ**: በ ውስጥ `NoritoRpcClient` እና `NoritoRpcRequestOptions` ይጠቀሙ
  አንድሮይድ ኤስዲኬ
- ** ጃቫ ስክሪፕት / ስዊፍት ***: ረዳቶች በ `docs/source/torii/norito_rpc_tracker.md` ውስጥ ክትትል ይደረግባቸዋል
  እና እንደ NRPC-3 አካል ያርፋል።

## የኮንሶል ናሙና ይሞክሩት።

ገምጋሚዎች Norito ን እንደገና ማጫወት እንዲችሉ የገንቢው መግቢያው የሙከራ ፕሮክሲን ይልካል።
የተገመቱ ስክሪፕቶችን ሳይጽፉ የሚጫኑ ጭነቶች።

1. [ተኪውን ጀምር](./try-it.md#start-the-proxy-locally) እና አዘጋጅ
   `TRYIT_PROXY_PUBLIC_URL` ስለዚህ መግብሮቹ ትራፊክ የት እንደሚልኩ ያውቃሉ።
2. ** ሞክሩት *** ካርዱን በዚህ ገጽ ወይም `/reference/torii-swagger` ይክፈቱ
   ፓነል እና እንደ `POST /v2/pipeline/submit` ያለ የመጨረሻ ነጥብ ይምረጡ።
3. **የይዘት አይነት** ወደ `application/x-norito` ይቀይሩ፣ **ሁለትዮሽ** ይምረጡ።
   አርታዒ፣ እና `fixtures/norito_rpc/transfer_asset.norito` ስቀል
   (ወይም ማንኛውም የተጫነ ጭነት
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`)።
4. በOAuth መሳሪያ-ኮድ መግብር ወይም በእጅ ማስመሰያ በኩል ተሸካሚ ማስመሰያ ያቅርቡ
   መስክ (ተኪው ሲዋቀር I18NI0000042X መሻሮችን ይቀበላል
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. ጥያቄውን ያስገቡ እና Torii በ ውስጥ የተዘረዘሩትን `schema_hash` የሚያስተጋባ መሆኑን ያረጋግጡ
   `fixtures/norito_rpc/schema_hashes.json`. ተዛማጅ hashes ያረጋግጣሉ
   Norito ራስጌ ከአሳሽ/ፕሮክሲ ሆፕ ተረፈ።

ለፍኖተ ካርታ ማስረጃ፣ ይሞክሩት ቅጽበታዊ ገጽ እይታን ከአንድ ሩጫ ጋር ያጣምሩት።
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. ስክሪፕቱ ይጠቀለላል
`cargo xtask norito-rpc-verify`፣ የJSON ማጠቃለያውን ይጽፋል
`artifacts/norito_rpc/<timestamp>/`, እና ተመሳሳይ መጋጠሚያዎችን ይይዛል
ፖርታል ተበላ።

## መላ መፈለግ

| ምልክት | የት ይታያል | ምክንያት | አስተካክል |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii ምላሽ | የጠፋ ወይም የተሳሳተ I18NI0000050X ራስጌ | ክፍያውን ከመላክዎ በፊት `Content-Type: application/x-norito` ያዘጋጁ። |
| `X-Iroha-Error-Code: schema_mismatch` (ኤችቲቲፒ 400) | Torii ምላሽ አካል / ራስጌዎች | Fixture schema hash ከ Torii ግንባታ ይለያል | መገልገያዎችን በ I18NI0000053X ያድሱ እና በ `fixtures/norito_rpc/schema_hashes.json` ውስጥ ያለውን ሃሽ ያረጋግጡ; የመጨረሻው ነጥብ Norito ካልነቃ ወደ JSON ይመለሱ። |
| `{"error":"origin_forbidden"}` (ኤችቲቲፒ 403) | ይሞክሩት የተኪ ምላሽ | ጥያቄ የመጣው በ`TRYIT_PROXY_ALLOWED_ORIGINS` ውስጥ ካልተዘረዘረ መነሻ ነው። የፖርታል መነሻውን (ለምሳሌ `https://docs.devnet.sora.example`) ወደ env var ያክሉ እና ፕሮክሲውን እንደገና ያስጀምሩ። |
| `{"error":"rate_limited"}` (ኤችቲቲፒ 429) | ይሞክሩት የተኪ ምላሽ | የየአይ ፒ ኮታ ከI18NI0000059X/`TRYIT_PROXY_RATE_WINDOW_MS` በጀት አልፏል | የውስጥ ጭነት ሙከራ ገደቡን ይጨምሩ ወይም መስኮቱ ዳግም እስኪጀምር ድረስ ይጠብቁ (በJSON ምላሽ ውስጥ I18NI0000061X ይመልከቱ)። |
| `{"error":"upstream_timeout"}` (ኤችቲቲፒ 504) ወይም `{"error":"upstream_error"}` (ኤችቲቲፒ 502) | ይሞክሩት የተኪ ምላሽ | Torii ጊዜው አልፎበታል ወይም ተኪው የተዋቀረውን የኋላ ክፍል መድረስ አልቻለም | `TRYIT_PROXY_TARGET` ሊደረስበት የሚችል መሆኑን ያረጋግጡ፣ የTorii ጤናን ያረጋግጡ ወይም በትልቁ `TRYIT_PROXY_TIMEOUT_MS` እንደገና ይሞክሩ። |

ተጨማሪ ይሞክሩት መመርመሪያዎች እና OAuth ምክሮች በቀጥታ ውስጥ ይኖራሉ
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples)።

## ተጨማሪ መገልገያዎች
- የመጓጓዣ RFC: I18NI0000067X
- አስፈፃሚ ማጠቃለያ: I18NI0000068X
- የድርጊት መከታተያ: `docs/source/torii/norito_rpc_tracker.md`
- ተኪ መመሪያዎችን ይሞክሩ-`docs/portal/docs/devportal/try-it.md`