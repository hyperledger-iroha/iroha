---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bb0965d4125aa2c3a3d483b63f4b36b1c6bf26406a2fd54e645e7a3c0156c264
source_last_modified: "2026-01-05T09:28:11.906678+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ባለብዙ ምንጭ አቅራቢ ማስታወቂያዎች እና መርሐግብር

ይህ ገጽ ቀኖናዊውን ዝርዝር ወደ ውስጥ ያሰራጫል።
[`docs/source/sorafs/provider_advert_multisource.md`](I18NU0000009X)።
ያንን ሰነድ በቃል ለ I18NT0000001X እቅዶች እና የለውጥ ሎግዎች ይጠቀሙ። የፖርታል ቅጂው
የኦፕሬተር መመሪያን፣ የኤስዲኬ ማስታወሻዎችን እና የቴሌሜትሪ ማጣቀሻዎችን ከቀሪው ጋር ያስቀምጣል።
የ SoraFS runbooks.

## Norito የመርሃግብር ተጨማሪዎች

### ክልል አቅም (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - በጥያቄ ትልቁ የተከታታይ ስፋት (ባይት)፣ `≥ 1`።
- `min_granularity` - ጥራት ይፈልጉ ፣ I18NI0000015X።
- `supports_sparse_offsets` - በአንድ ጥያቄ ውስጥ ቀጣይ ያልሆኑ ማካካሻዎችን ይፈቅዳል።
- `requires_alignment` - እውነት ሲሆን ማካካሻዎች ከ`min_granularity` ጋር መጣጣም አለባቸው።
- `supports_merkle_proof` - የPoR ምስክር ድጋፍን ያመለክታል።

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` ቀኖናዊ ኢንኮዲንግ ያስፈጽማል
ስለዚህ የሀሜት ሸክሞች ቆራጥነት ይቀራሉ።

### `StreamBudgetV1`
- መስኮች: I18NI0000023X, I18NI0000024X, አማራጭ I18NI0000025X.
- የማረጋገጫ ደንቦች (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`፣ `max_bytes_per_sec > 0`።
  - `burst_bytes`፣ ሲገኝ፣ `> 0` እና `≤ max_bytes_per_sec` መሆን አለበት።

### `TransportHintV1`
መስኮች፡- `protocol: TransportProtocol`፣ `priority: u8` (0–15 መስኮት በ
  `TransportHintV1::validate`).
- የታወቁ ፕሮቶኮሎች፡ `torii_http_range`፣ `quic_stream`፣ `soranet_relay`፣
  `vendor_reserved`.
- በአንድ አቅራቢ የተባዙ የፕሮቶኮል ግቤቶች ውድቅ ተደርገዋል።

### `ProviderAdvertBodyV1` ተጨማሪዎች
- አማራጭ `stream_budget: Option<StreamBudgetV1>`.
- አማራጭ I18NI0000042X.
- ሁለቱም መስኮች አሁን በ I18NI0000043X, አስተዳደር በኩል ይፈስሳሉ
  ኤንቨሎፕ፣ የCLI እቃዎች እና ቴሌሜትሪክ JSON።

## የማረጋገጫ እና የአስተዳደር አስገዳጅነት

`ProviderAdvertBodyV1::validate` እና `ProviderAdmissionProposalV1::validate`
የተበላሸ ዲበ ውሂብን ውድቅ ያድርጉ፡

- የክልሎች አቅም መፍታት እና የስፋት/የጥራጥሬ ገደቦችን ማርካት አለበት።
- የዥረት በጀቶች / የመጓጓዣ ፍንጮች ተዛማጅ ያስፈልጋቸዋል
  `CapabilityType::ChunkRangeFetch` TLV እና ባዶ ያልሆነ ፍንጭ ዝርዝር።
- የተባዙ የትራንስፖርት ፕሮቶኮሎች እና ልክ ያልሆኑ ቅድሚያዎች ማረጋገጫን ያሳድጋሉ።
  ማስታወቂያ ከመወራቱ በፊት ስህተቶች።
- የመግቢያ ፖስታዎች ለክልል ሜታዳታ ፕሮፖዛል/ማስታወቂያ ያወዳድራሉ
  `compare_core_fields` ስለዚህ የማይዛመዱ የሃሜት ጭነቶች ቀደም ብለው ውድቅ ይደረጋሉ።

የድጋፍ ሽፋን ይኖራል
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## መሳሪያ እና እቃዎች

- የአቅራቢዎች የማስታወቂያ ጭነቶች `range_capability`፣ `stream_budget`፣ እና ማካተት አለባቸው።
  `transport_hints` ሜታዳታ። በI18NI0000052X ምላሾች በኩል ያረጋግጡ እና
  የመግቢያ ዕቃዎች; የJSON ማጠቃለያዎች የተተነተነውን አቅም ማካተት አለባቸው፣
  የዥረት በጀት፣ እና ለቴሌሜትሪ መግቢያ ፍንጭ ድርድሮች።
- `cargo xtask sorafs-admission-fixtures` የወለል ንጣፎች በጀቶችን እና መጓጓዣዎችን ያሰራጫሉ።
  ዳሽቦርዶች የማደጎ ባህሪን ለመከታተል በJSON ቅርሶቹ ውስጥ ፍንጭ ይሰጣል።
- በ `fixtures/sorafs_manifest/provider_admission/` ስር ያሉ ቋሚዎች አሁን ያካትታሉ፡
  - ቀኖናዊ ባለብዙ ምንጭ ማስታወቂያዎች ፣
  - `multi_fetch_plan.json` ስለዚህ የኤስዲኬ ስብስቦች የሚወስን ባለብዙ-አቻን እንደገና ማጫወት ይችላሉ
    እቅድ ማውጣት.

## ኦርኬስትራ እና Torii ውህደት

- Torii `/v2/sorafs/providers` የተተነተነ ክልል አቅም ሜታዳታ አብሮ ይመልሳል
  ከ `stream_budget` እና `transport_hints` ጋር። የማውረድ ማስጠንቀቂያዎች ሲቃጠሉ
  አቅራቢዎች አዲሱን ሜታዳታ ይተዉታል፣ እና የመተላለፊያ መንገዱ የመጨረሻ ነጥቦችም ይህንኑ ያስገድዳሉ
  ለቀጥታ ደንበኞች ገደቦች.
- ባለብዙ ምንጭ ኦርኬስትራ (`sorafs_car::multi_fetch`) አሁን ክልልን ያስፈጽማል
  ሥራ በሚመድቡበት ጊዜ ገደቦች፣ የችሎታ አሰላለፍ እና የዥረት በጀት ማውጣት። ክፍል
  ፈተናዎች ቁርጥራጭ-በጣም-ትልቅ፣ በጥቂቱ-መፈለግ እና በጉሮሮ ውስጥ ያሉ ሁኔታዎችን ይሸፍናሉ።
- `sorafs_car::multi_fetch` ዥረቶች ወደ ታች የማውረድ ምልክቶች (የአሰላለፍ ውድቀቶች፣
  ስሮትልድ ጥያቄዎች) ስለዚህ ኦፕሬተሮች የተወሰኑ አቅራቢዎች ለምን እንደነበሩ ማወቅ ይችላሉ።
  በእቅድ ጊዜ ተዘሏል.

## ቴሌሜትሪ ማጣቀሻ

የTorii ክልል ማምጫ መሳሪያ የ*SoraFS ፈልጎ ታዛቢነትን ይመገባል።
Grafana ዳሽቦርድ (`dashboards/grafana/sorafs_fetch_observability.json`) እና እ.ኤ.አ.
የተጣመሩ የማንቂያ ደንቦች (I18NI0000062X)።

| መለኪያ | አይነት | መለያዎች | መግለጫ |
|--------|-------|----|------------|
| `torii_sorafs_provider_range_capability_total` | መለኪያ | `feature` (`providers`፣ `supports_sparse_offsets`፣ `requires_alignment`፣ `supports_merkle_proof`፣ `stream_budget`፣ I18NI0000070X) | አቅራቢዎች የማስታወቂያ ክልል ችሎታ ባህሪያት. |
| `torii_sorafs_range_fetch_throttle_events_total` | ቆጣሪ | `reason` (`quota`፣ `concurrency`፣ `byte_rate`) | ስሮትልድ ክልል ለማምጣት ሙከራዎች በመመሪያ ተቧድነው። |
| `torii_sorafs_range_fetch_concurrency_current` | መለኪያ | - | የጋራ የተዛማጅ በጀት የሚበሉ ንቁ የተጠበቁ ዥረቶች። |

ምሳሌ PromQL ቅንጥቦች፡

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

ከማንቃትዎ በፊት የኮታ ማስፈጸሙን ለማረጋገጥ ስሮትል ቆጣሪውን ይጠቀሙ
የብዝሃ-ምንጭ ኦርኬስትራ ነባሪዎች፣ እና ተጓዳኝ ሲቃረብ ያስጠነቅቁ
የጅረት በጀት ከፍተኛውን ለእርስዎ መርከቦች።