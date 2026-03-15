---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e0cdd8242b45628e688d94ebec08e2d9900787ec93a81417e6683d399d43be2d
source_last_modified: "2026-01-22T14:35:36.781385+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS መስቀለኛ መንገድ ↔ የደንበኛ ፕሮቶኮል

ይህ መመሪያ ቀኖናዊውን የፕሮቶኮል ፍቺ በ ውስጥ ያጠቃልላል
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)።
ለባይት ደረጃ Norito አቀማመጦች እና የለውጥ ሎግዎች ወደ ላይ ያለውን ዝርዝር ተጠቀም፤ ፖርታሉ
ኮፒ የተግባር ድምቀቶችን ከቀሪዎቹ የI18NT0000006X Runbooks ጋር ያቆያል።

## የአቅራቢ ማስታወቂያዎች እና ማረጋገጫ

SoraFS አቅራቢዎች ወሬ `ProviderAdvertV1` ክፍያ (ይመልከቱ)
`crates/sorafs_manifest::provider_advert`) በሚተዳደረው ኦፕሬተር የተፈረመ።
ማስታወቂያዎቹ የግኝት ሜታዳታ ፒን እና የጥበቃ መንገዶች ባለብዙ ምንጭ
ኦርኬስትራ በሥራ ሰዓት ያስገድዳል።

- ** የህይወት ዘመን *** - `issued_at < expires_at ≤ issued_at + 86 400 s`. አቅራቢዎች
  በየ 12 ሰዓቱ ማደስ አለበት።
- ** አቅም TLVs *** — የ TLV ዝርዝር የትራንስፖርት ባህሪያትን ያስተዋውቃል (Torii፣
  QUIC+Noise፣ SoraNet relays፣ የአቅራቢ ቅጥያዎች)። ያልታወቁ ኮዶች ሊዘለሉ ይችላሉ።
  GREASE መመሪያን በመከተል `allow_unknown_capabilities = true` ጊዜ።
- ** QoS ፍንጮች *** - `availability` ደረጃ (ሙቅ/ሙቅ/ቀዝቃዛ)፣ ከፍተኛ ሰርስሮ ማውጣት
  የቆይታ ጊዜ፣ የተዛማጅነት ገደብ እና አማራጭ የዥረት በጀት። QoS ጋር መመሳሰል አለበት።
  የታየ ቴሌሜትሪ እና በመግቢያ ኦዲት ይደረጋል።
- ** የመጨረሻ ነጥቦች እና አስደሳች ርዕሶች *** - ተጨባጭ አገልግሎት ዩአርኤሎች ከTLS/ALPN ጋር
  ሜታዳታ እና ደንበኞች በሚገነቡበት ጊዜ መመዝገብ ያለባቸው የግኝት ርዕሶች
  የጥበቃ ስብስቦች.
- **የዱካ ብዝሃነት ፖሊሲ** — `min_guard_weight`፣ AS/ፑል ደጋፊ መውጫ ካፕ፣ እና
  `provider_failure_threshold` ቆራጥ የብዝሃ-አቻዎችን ማምጣት ይቻላል።
- **የመገለጫ ለዪዎች** — አቅራቢዎች ቀኖናዊውን እጀታ ማጋለጥ አለባቸው (ለምሳሌ፡.
  `sorafs.sf1@1.0.0`); አማራጭ `profile_aliases` የቆዩ ደንበኞች እንዲሰደዱ ያግዛል።

የማረጋገጫ ደንቦች ዜሮ ድርሻን፣ ባዶ አቅም/የመጨረሻ ነጥብ/የርዕስ ዝርዝሮችን ውድቅ ያደርጋሉ፣
የተሳሳተ የህይወት ዘመን፣ ወይም የጎደሉ የQoS ኢላማዎች። የመግቢያ ኤንቨሎፖች ከ
የማስታወቂያ እና ፕሮፖዛል አካላት (`compare_core_fields`) ከሐሜት ዝመናዎች በፊት።

### ክልል ማምጣት ቅጥያዎች

ክልል-የሚችሉ አቅራቢዎች የሚከተለውን ሜታዳታ ያካትታሉ፡

| መስክ | ዓላማ |
|-------|--------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`፣ I18NI0000029X፣ እና አሰላለፍ/ማስረጃ ባንዲራዎችን ያውጃል። |
| `StreamBudgetV1` | የአማራጭ ተመሳሳይነት/የወጪ ፖስታ (`max_in_flight`፣ `max_bytes_per_sec`፣ አማራጭ `burst`)። ክልል አቅምን ይፈልጋል። |
| `TransportHintV1` | የታዘዙ የመጓጓዣ ምርጫዎች (ለምሳሌ፣ `torii_http_range`፣ I18NI0000036X፣ `soranet_relay`)። ቅድሚያ የሚሰጣቸው ነገሮች I18NI0000038X ሲሆኑ የተባዙት ውድቅ ናቸው። |

የመሳሪያ ድጋፍ;

- የአቅራቢዎች የማስታወቂያ ቧንቧዎች የክልል አቅም፣ የዥረት በጀት እና እና ማረጋገጥ አለባቸው
  ለኦዲት ቆራጥ ሸክሞችን ከማውጣቱ በፊት ፍንጮችን ማጓጓዝ።
- `cargo xtask sorafs-admission-fixtures` ቀኖናዊ ባለብዙ-ምንጭ ቅርቅቦች
  ከስር ማሽቆልቆል ዕቃዎች ጎን ለጎን ያስተዋውቃል
  `fixtures/sorafs_manifest/provider_admission/`.
- `stream_budget` ወይም `transport_hints`ን የሚተዉ ክልል-የሚችሉ ማስታወቂያዎች
  መርሐግብር ከመያዙ በፊት በCLI/SDK ሎደሮች ውድቅ ተደርጓል፣ ባለብዙ-ምንጩን ይጠብቃል።
  ከ Torii የመግቢያ ተስፋዎች ጋር የተጣጣመ መታጠቂያ።

## የጌትዌይ ክልል የመጨረሻ ነጥቦች

ጌትዌይስ የማስታወቂያ ዲበ ውሂቡን የሚያንፀባርቁ ወሳኝ የኤችቲቲፒ ጥያቄዎችን ይቀበላሉ።

### `GET /v2/sorafs/storage/car/{manifest_id}`

| መስፈርት | ዝርዝሮች |
|------------|-------|
| **ራስጌዎች** | `Range` (አንድ መስኮት ከ chunk offsets ጋር የተስተካከለ)፣ `dag-scope: block`፣ `X-SoraFS-Chunker`፣ አማራጭ `X-SoraFS-Nonce`፣ እና አስገዳጅ base64 `X-SoraFS-Stream-Token`። |
| **ምላሾች** | `206` ከ `Content-Type: application/vnd.ipld.car` ጋር፣ `Content-Range` የሚቀርበውን መስኮት የሚገልጽ `X-Sora-Chunk-Range` ሜታዳታ እና chunker/token ራስጌዎችን አስተጋባ። |
| ** ውድቀት ሁነታዎች *** | `416` ለተሳሳቱ ክልሎች፣ `401` ለጠፉ/ልክ ያልሆኑ ቶከኖች፣ የዥረት/ባይት በጀቶች ሲያልፍ `429`። |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

ነጠላ-ቸንክ ማምጣት ከተመሳሳዩ ራስጌዎች እና ቆራጥ ቁርጥራጭ መፍጨት።
የCAR ቁርጥራጭ አላስፈላጊ በማይሆንበት ጊዜ ለዳግም ሙከራዎች ወይም ለፍርድ ማውረዶች ይጠቅማል።

## ባለብዙ ምንጭ ኦርኬስትራ የስራ ፍሰት

SF-6 ባለብዙ-ምንጭ ማምጣት ሲነቃ (Rust CLI በ I18NI0000057X፣
ኤስዲኬዎች በI18NI0000058X በኩል፡

1. ** ግብዓቶችን ሰብስብ *** - የማሳያውን ክፍል መፍታት ፣ የቅርብ ጊዜ ማስታወቂያዎችን ጎትት ፣
   እና እንደ አማራጭ የቴሌሜትሪ ቅጽበተ-ፎቶን (`--telemetry-json` ወይም
   `TelemetrySnapshot`)።
2. ** የውጤት ሰሌዳ ይገንቡ *** - `Orchestrator::build_scoreboard` ይገመግማል
   ብቁነት እና ውድቅ የተደረጉ ምክንያቶችን ይመዘግባል; `sorafs_fetch --scoreboard-out`
   JSON እንደቀጠለ ነው።
3. ** የጊዜ መርሐግብር ቁርጥራጮች** — `fetch_with_scoreboard` (ወይም `--plan`) ክልል ያስገድዳል።
   ገደቦች፣ የዥረት በጀቶች፣ ድጋሚ ይሞክሩ/የአቻ ካፕ (I18NI0000065X፣
   `--max-peers`)፣ እና ለእያንዳንዱ ጥያቄ አንጸባራቂ ስፋት ያለው የዥረት ማስመሰያ ያወጣል።
4. ** ደረሰኞችን ያረጋግጡ *** - ውጤቱ `chunk_receipts` እና
   `provider_reports`; የ CLI ማጠቃለያዎች `provider_reports` ይቀጥላሉ፣
   `chunk_receipts`፣ እና `ineligible_providers` ለማስረጃ ጥቅሎች።

ወደ ኦፕሬተሮች/ኤስዲኬዎች የተነሱ የተለመዱ ስህተቶች፡-

| ስህተት | መግለጫ |
|-------|-----------|
| `no providers were supplied` | ከተጣራ በኋላ ምንም ብቁ የሆኑ ግቤቶች የሉም። |
| `no compatible providers available for chunk {index}` | የአንድ የተወሰነ ክፍል ክልል ወይም የበጀት አለመመጣጠን። |
| `retry budget exhausted after {attempts}` | `--retry-budget` ይጨምሩ ወይም ያልተሳኩ አቻዎችን ያስወጡ። |
| `no healthy providers remaining` | ከተደጋጋሚ ውድቀቶች በኋላ ሁሉም አቅራቢዎች ተሰናክለዋል። |
| `streaming observer failed` | የታችኛው CAR ጸሐፊ ተቋርጧል። |
| `orchestrator invariant violated` | አንጸባራቂ፣ የውጤት ሰሌዳ፣ የቴሌሜትሪ ቅጽበታዊ ገጽ እይታ እና CLI JSON ለሙከራ ያንሱ። |

## ቴሌሜትሪ እና ማስረጃ

- በኦርኬስትራ የወጡ መለኪያዎች፡-  
  `sorafs_orchestrator_active_fetches`፣ `sorafs_orchestrator_fetch_duration_ms`፣
  `sorafs_orchestrator_retries_total`፣ `sorafs_orchestrator_provider_failures_total`
  (በአንጸባራቂ/ክልል/አቅራቢ ተሰጥቷል)። `telemetry_region` በማዋቀር ወይም በ በኩል ያቀናብሩ
  CLI ባንዲራዎች ስለዚህ ዳሽቦርዶች መርከቦች ክፍልፍል.
- የCLI/SDK ማምጣት ማጠቃለያዎች የማያቋርጥ የውጤት ሰሌዳ JSON፣ ቸንክ ደረሰኞች፣
  እና ለSF-6/SF-7 በሮች በታቀደ ጥቅሎች መላክ ያለባቸው አቅራቢዎች ሪፖርት ያደርጋሉ።
- የጌትዌይ ተቆጣጣሪዎች `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` ያጋልጣሉ
  ስለዚህ SRE ዳሽቦርዶች የኦርኬስትራ ውሳኔዎችን ከአገልጋይ ባህሪ ጋር ማዛመድ ይችላሉ።

## CLI እና REST አጋዦች

- `iroha app sorafs pin list|show`፣ `alias list`፣ እና `replication list` ይጠቀለላል
  ፒን መዝገብ REST የመጨረሻ ነጥቦችን እና ጥሬ Norito JSON ን ከማስረጃ ብሎኮች ጋር ያትሙ
  ለኦዲት ማስረጃ.
- `iroha app sorafs storage pin` እና `torii /v2/sorafs/pin/register` Norito ይቀበላሉ
  ወይም JSON ይገለጻል እና አማራጭ ተለዋጭ ስም ማረጋገጫዎች እና ተተኪዎች። የተበላሹ ማስረጃዎች
  `400` ያሳድጉ፣ የቆየ ማረጋገጫዎች ላዩን `503` ከ `Warning: 110` ጋር፣ እና
  ጠንካራ ጊዜ ያለፈባቸው ማረጋገጫዎች `412` ይመለሳሉ።
- `iroha app sorafs repair list` መስተዋቶች ጥገና ወረፋ ማጣሪያዎች, ሳለ
  `repair claim|complete|fail|escalate` የተፈረመ የሰራተኛ ድርጊቶችን ወይም መቆራረጥን ያቀርባል
  የውሳኔ ሃሳቦች ለ Torii. Slash ፕሮፖዛል የአስተዳደር ማጽደቅ ማጠቃለያን ሊያካትቱ ይችላሉ።
  (የድምጽ ቆጠራዎችን ማጽደቅ/ አለመቀበል/አታቀብ እና የጸደቀ_በ/የተጠናቀቀ_በ
  የጊዜ ማህተም); ሲገኝ ምልአተ ጉባኤን ማሟላት እና የክርክር/ይግባኝ መስኮቶችን ማሟላት አለበት፣
  አለበለዚያ ድምጾች በመጨረሻው ቀን እስኪፈቱ ድረስ ፕሮፖዛሉ በክርክር ውስጥ ይቆያል.
- የጥገና ዝርዝሮች እና የሰራተኛ ወረፋ ምርጫ በ SLA ቀነ-ገደብ፣ ውድቀቶች ክብደት እና የአቅራቢዎች የኋላ መዝገብ በቆራጥ ማያያዣ ሰሪዎች (የወረፋ ጊዜ፣ የሰነድ መግለጫ፣ የቲኬት መታወቂያ) የታዘዙ ናቸው።
- የጥገና ሁኔታ ምላሾች ቤዝ64 I18NT0000003X የያዘ `events` ድርድር ያካትታል
  `RepairTaskEventV1` ግቤቶች ለኦዲት ዱካዎች በአጋጣሚ የታዘዙ; ዝርዝሩን
  ወደ የቅርብ ጊዜ ሽግግሮች ተወስኗል።
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` ተነባቢ-ብቻ ያመነጫል።
  ለኦዲት ማስረጃ ከአካባቢው የሰነድ ዝርዝር ማከማቻ ሪፖርቶች።
- REST የመጨረሻ ነጥቦች (`/v2/sorafs/pin`፣ `/v2/sorafs/aliases`፣
  `/v2/sorafs/replication`) ደንበኞች እንዲችሉ የማረጋገጫ መዋቅሮችን ያካትታል
  እርምጃ ከመውሰዳችሁ በፊት መረጃዎችን ከቅርብ ጊዜዎቹ ብሎክ ራስጌዎች ጋር ያረጋግጡ።

## ዋቢዎች

- ቀኖናዊ ዝርዝር:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito ዓይነቶች፡ `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI ረዳቶች: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- ኦርኬስትራ ሣጥን: `crates/sorafs_orchestrator`
- ዳሽቦርድ ጥቅል: `dashboards/grafana/sorafs_fetch_observability.json`