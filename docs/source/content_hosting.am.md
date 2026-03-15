---
lang: am
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% የይዘት ማስተናገጃ መስመር
% Iroha ኮር

# የይዘት ማስተናገጃ መስመር

የይዘቱ መስመር ትናንሽ የማይንቀሳቀሱ ቅርቅቦችን (ታር መዛግብትን) በሰንሰለት ላይ ያከማቻል እና ያገለግላል
የግለሰብ ፋይሎች በቀጥታ ከ Torii.

- ** አትም ***: `PublishContentBundle` ከታር መዝገብ ጋር ያቅርቡ፣ አማራጭ የማለቂያ ጊዜ
  ቁመት, እና አማራጭ አንጸባራቂ. የጥቅል መታወቂያው የ blake2b ሃሽ ነው።
  ታርቦል. የ Tar ግቤቶች መደበኛ ፋይሎች መሆን አለባቸው; ስሞች መደበኛ UTF-8 ዱካዎች ናቸው።
  መጠን/ዱካ/የፋይል ቆጠራ ካፕ ከ`content` ውቅር (`max_bundle_bytes`፣
  `max_files`፣ `max_path_len`፣ `max_retention_blocks`፣ `chunk_size_bytes`)።
  መግለጫዎች Norito-index hash፣ dataspace/ሌን፣ መሸጎጫ ፖሊሲን ያካትታሉ።
  (`max_age_seconds`፣ `immutable`)፣ የ auth ሁነታ (`public` / `role:<role>` /
  `sponsor:<uaid>`)፣ የማቆያ ፖሊሲ ቦታ ያዥ እና MIME ይሽራል።
- ** መቀነስ ***: የ tar ክፍያ ጭነቶች ተሰባብረዋል (ነባሪ 64KiB) እና በያንዳንዱ አንድ ጊዜ ይከማቻሉ
  ሃሽ ከማጣቀሻ ቁጥሮች ጋር; አንድ ጥቅል እየቀነሰ እና የፕሪም ቁርጥራጮች ጡረታ መውጣት።
- ** አገልግሉ ***: Torii `GET /v2/content/{bundle}/{path}` ያጋልጣል። የምላሾች ዥረት
  በቀጥታ ከ chunk store `ETag` = file hash፣ `Accept-Ranges: bytes`፣
  ክልል ድጋፍ፣ እና መሸጎጫ-ቁጥጥር ከማንፀባረቂያው የተገኘ። ያነባል።
  አንጸባራቂ የድጋፍ ሁነታ፡ ሚና-የተከለለ እና ስፖንሰር-የተከለሉ ምላሾች ቀኖናዊ ያስፈልጋቸዋል
  ለተፈረመው ራስጌዎች (`X-Iroha-Account`, `X-Iroha-Signature`) ይጠይቁ
  መለያ; የጎደሉ/ያለባቸው ጥቅሎች ተመላሽ 404.
- ** CLI ***: `iroha content publish --bundle <path.tar>` (ወይም `--root <dir>`) አሁን
  አንጸባራቂን በራስ ሰር ያመነጫል፣ አማራጭ `--manifest-out/--bundle-out` ያወጣል፣ እና
  `--auth`፣ `--cache-max-age-secs`፣ `--dataspace`፣ `--lane`፣ `--immutable`፣
  እና `--expires-at-height` ይሽራል። `iroha content pack --root <dir>` ይገነባል።
  ምንም ነገር ሳያስገቡ የሚወስን ታርቦል + አንጸባራቂ።
- ** ማዋቀር ***: መሸጎጫ/የማስመሪያ ቁልፎች በ `content.*` በ `iroha_config` ውስጥ ይኖራሉ
  (`default_cache_max_age_secs`፣ `max_cache_max_age_secs`፣ `immutable_bundles`፣
  `default_auth_mode`) እና በህትመት ጊዜ ተፈጻሚነት ይኖራቸዋል።
- ** SLO + ገደቦች ***: `content.max_requests_per_second` / `request_burst` እና
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` ካፕ ተነባቢ ጎን
  የመተላለፊያ መንገድ; Torii ባይት እና ወደ ውጭ ከመላክ በፊት ሁለቱንም ያስፈጽማል
  `torii_content_requests_total`፣ `torii_content_request_duration_seconds`፣ እና
  `torii_content_response_bytes_total` መለኪያዎች ከውጤት መለያዎች ጋር። መዘግየት
  ኢላማዎች በ `content.target_p50_latency_ms` / ስር ይኖራሉ
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- ** አላግባብ መጠቀም መቆጣጠሪያዎች ***: ተመን ባልዲዎች በ UAID/API token/የርቀት አይፒ ተከፍተዋል እና
  አማራጭ PoW ጠባቂ (`content.pow_difficulty_bits`፣ `content.pow_header`) ይችላል
  ከማንበብ በፊት ያስፈልጋል. የዲኤ ስትሪፕ አቀማመጥ ነባሪዎች የመጡ ናቸው።
  `content.stripe_layout` እና ደረሰኞች ላይ ተስተጋብተዋል።
- ** ደረሰኞች እና የዲኤ ማስረጃ ***: የተሳካላቸው ምላሾች ተያይዘዋል
  `sora-content-receipt` (base64 Norito-framed `ContentDaReceipt` ባይት) ይዞ
  `bundle_id`፣ `path`፣ `file_hash`፣ `served_bytes`፣ የቀረበው ባይት ክልል፣
  `chunk_root` / `stripe_layout`፣ አማራጭ የ PDP ቁርጠኝነት እና የጊዜ ማህተም እንዲሁ
  ደንበኞቹ አካሉን እንደገና ሳያነቡ የተገኘውን ነገር መሰካት ይችላሉ።

ቁልፍ ማጣቀሻዎች፡-- የውሂብ ሞዴል: `crates/iroha_data_model/src/content.rs`
- አፈጻጸም: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ተቆጣጣሪ፡ `crates/iroha_torii/src/content.rs`
- CLI አጋዥ: `crates/iroha_cli/src/content.rs`