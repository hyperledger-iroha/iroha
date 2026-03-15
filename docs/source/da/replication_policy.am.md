---
lang: am
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የውሂብ ተገኝነት ማባዛት ፖሊሲ (DA-4)

_ሁኔታ፡ በሂደት ላይ - ባለቤቶች፡ ኮር ፕሮቶኮል WG/የማከማቻ ቡድን/SRE_

የዲኤ ማስገቢያ ቱቦ አሁን ቆራጥ የማቆየት ኢላማዎችን ያስፈጽማል
በ`roadmap.md` (የስራ ዥረት DA-4) የተገለፀው እያንዳንዱ የብሎብ ክፍል። Torii እምቢ አለ።
ከተዋቀረው ጋር የማይዛመድ በጥሪ የቀረቡ ማቆያ ኤንቨሎፖች ቀጣይነት ያለው
ፖሊሲ፣ እያንዳንዱ አረጋጋጭ/የማከማቻ መስቀለኛ መንገድ አስፈላጊውን እንደሚይዝ ዋስትና ይሰጣል
በአስረካቢው ሀሳብ ላይ ሳይመሰረቱ የዘመናት እና ቅጂዎች ብዛት።

## ነባሪ ፖሊሲ

| የብሎብ ክፍል | ትኩስ ማቆየት | ቀዝቃዛ ማቆየት | የሚፈለጉ ቅጂዎች | የማከማቻ ክፍል | አስተዳደር መለያ |
|------------------
| `taikai_segment` | 24 ሰአት | 14 ቀናት | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ሰአት | 7 ቀናት | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ሰአት | 180 ቀናት | 3 | `cold` | `da.governance` |
| _ነባሪ (ሁሉም ሌሎች ክፍሎች) _ | 6 ሰአት | 30 ቀናት | 3 | `warm` | `da.default` |

እነዚህ እሴቶች በ`torii.da_ingest.replication_policy` ውስጥ የተካተቱ እና ተግባራዊ ናቸው።
ሁሉም `/v2/da/ingest` ማቅረቢያዎች. Torii እንደገና ይጽፋል ከተገደዱ ጋር
ፕሮፋይሉን ማቆየት እና ደዋዮች የማይዛመዱ እሴቶችን ሲያቀርቡ ማስጠንቀቂያ ይሰጣል
ኦፕሬተሮች ያረጁ ኤስዲኬዎችን ማወቅ ይችላሉ።

### የታይካይ ተደራሽነት ክፍሎች

የታይካይ ማዘዋወር መግለጫዎች (`taikai.trm` ሜታዳታ) አሁን የሚከተሉትን ያጠቃልላል
`availability_class` ፍንጭ (`Hot`፣ `Warm`፣ ወይም `Cold`)። በሚገኝበት ጊዜ፣ Torii
ተዛማጅ ማቆየት መገለጫ ከ `torii.da_ingest.replication_policy` ይመርጣል
የክስተት ኦፕሬተሮች እንቅስቃሴ-አልባነትን እንዲቀንሱ በማድረግ ክፍያውን ከመቁረጥዎ በፊት
የአለምአቀፍ የፖሊሲ ሠንጠረዥን ሳያርትዑ አተረጓጎም. ነባሪዎቹ፡-

| ተገኝነት ክፍል | ትኩስ ማቆየት | ቀዝቃዛ ማቆየት | የሚፈለጉ ቅጂዎች | የማከማቻ ክፍል | አስተዳደር መለያ |
|------------------
| `hot` | 24 ሰአት | 14 ቀናት | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ሰአት | 30 ቀናት | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ሰአት | 180 ቀናት | 3 | `cold` | `da.taikai.archive` |

አንጸባራቂው `availability_class`ን ከለቀቀ፣ የማስገቢያ መንገዱ ወደ ኋላ ይመለሳል።
የ`hot` መገለጫ ስለዚህ የቀጥታ ዥረቶች ሙሉ የቅጂ ስብስባቸውን እንዲጠብቁ። ኦፕሬተሮች ይችላሉ።
አዲሱን በማርትዕ እነዚህን እሴቶች ይሽሩ
`torii.da_ingest.replication_policy.taikai_availability` ማገጃ በማዋቀር።

## ማዋቀር

ፖሊሲው በ`torii.da_ingest.replication_policy` ስር ይኖራል እና አጋልጧል ሀ
*ነባሪ* አብነት እና የክፍል መሻሮች ድርድር። የክፍል መለያዎች ናቸው።
ለጉዳይ የማይሰማ እና `taikai_segment`፣ `nexus_lane_sidecar` መቀበል፣
`governance_artifact`፣ ወይም `custom:<u16>` በአስተዳደር ለጸደቁ ቅጥያዎች።
የማከማቻ ክፍሎች `hot`፣ `warm`፣ ወይም `cold` ይቀበላሉ።

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

ከላይ ከተዘረዘሩት ነባሪዎች ጋር ለመስራት ብሎኩን ሳይነካ ይተዉት። ለማጥበቅ ሀ
ክፍል, ተዛማጅ መሻርን ያዘምኑ; ለአዳዲስ ትምህርቶች መነሻውን ለመለወጥ ፣
`default_retention` አርትዕ.የተወሰኑ የታይካይ ተገኝነት ክፍሎችን ለማስተካከል ከስር ግቤቶችን ያክሉ
`torii.da_ingest.replication_policy.taikai_availability`፡

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## ማስፈጸሚያ ትርጉሞች

- Torii በተጠቃሚ የቀረበውን `RetentionPolicy` በተተገበረው መገለጫ ይተካዋል
  ልቀትን ከመቁረጥ ወይም ከማሳየት በፊት።
- ያልተዛመደ የማቆያ ፕሮፋይል ውድቅ መደረጉን የሚገልጹ ቀድሞ-የተገነቡ አንጸባራቂዎች
  ከ `400 schema mismatch` ጋር የቆዩ ደንበኞች ውሉን ማዳከም አይችሉም።
- እያንዳንዱ የመሻር ክስተት ተመዝግቧል (`blob_class`፣ ከተጠበቀው ፖሊሲ ጋር ገብቷል)
  በታቀደው ጊዜ ታዛዥ ያልሆኑ ደዋዮችን ለማሳየት።

ለተሻሻለው በር `docs/source/da/ingest_plan.md` (የማረጋገጫ ዝርዝር) ይመልከቱ
የማቆያ አፈፃፀምን የሚሸፍን.

## እንደገና ማባዛት የስራ ፍሰት (DA-4 ክትትል)

የማቆየት አፈፃፀም የመጀመሪያው እርምጃ ብቻ ነው። ኦፕሬተሮችም ያንን ማረጋገጥ አለባቸው
የቀጥታ መግለጫዎች እና የማባዛት ትዕዛዞች ከተዋቀረው መመሪያ ጋር አብረው ይቆያሉ።
SoraFS ከታዛዥነት ውጪ የሆኑ ነጠብጣቦችን በራስ ሰር እንደገና ሊደግም ይችላል።

1. ** ለመንሸራተት ተመልከት።** Torii የሚለቀቅ
   `overriding DA retention policy to match configured network baseline` በማንኛውም ጊዜ
   ደዋይ የቆዩ ማቆየት እሴቶችን ያቀርባል። ያንን ሎግ ያጣምሩት።
   `torii_sorafs_replication_*` ቴሌሜትሪ ድክመቶችን ወይም ዘግይቶ ለመለየት
   እንደገና ማሰማራት.
2. **ልዩ ዓላማ ከቀጥታ ቅጂዎች ጋር።** አዲሱን የኦዲት ረዳት ይጠቀሙ፡-

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   ትዕዛዙ `torii.da_ingest.replication_policy` ከተሰጠው ይጭናል
   ማዋቀር፣ እያንዳንዱን አንጸባራቂ (JSON ወይም Norito) መፍታት እና እንደ አማራጭ ከማንኛውም ጋር ይዛመዳል።
   `ReplicationOrderV1` ጭነቶች በማንፀባረቅ መፍጨት። ማጠቃለያው ሁለት ባንዲራ ነው።
   ሁኔታዎች፡-

   - `policy_mismatch` - አንጸባራቂ ማቆየት መገለጫው ከተገደደው ይለያል
     ፖሊሲ (Torii በተሳሳተ መንገድ ካልተዋቀረ በስተቀር ይህ በፍፁም መከሰት የለበትም)።
   - `replica_shortfall` - የቀጥታ ማባዛት ትዕዛዝ ያነሰ ቅጂዎችን ይጠይቃል
     `RetentionPolicy.required_replicas` ወይም ከእሱ ያነሱ ስራዎችን ያቀርባል
     ዒላማ.

   ዜሮ ያልሆነ የመውጫ ሁኔታ የ CI/የጥሪ አውቶማቲክ ገባሪ ጉድለትን ያሳያል
   ወዲያውኑ ገጽ ማድረግ ይችላል። የJSON ዘገባውን ከ
   ለፓርላማ ድምጾች `docs/examples/da_manifest_review_template.md` ጥቅል።
3. ** ድጋሚ ማባዛትን ቀስቅሰው።** ኦዲቱ ጉድለት እንዳለ ሲዘግብ አዲስ ያወጣል።
   `ReplicationOrderV1` በተገለጸው የአስተዳደር መሣሪያ በኩል
   `docs/source/sorafs/storage_capacity_marketplace.md` እና ኦዲቱን እንደገና አሂድ
   የተባዛው ስብስብ እስኪቀላቀል ድረስ. ለአደጋ ጊዜ መሻር፣ የCLI ውጤቱን ያጣምሩ
   SREs ተመሳሳዩን መፈጨት ማጣቀስ እንዲችሉ ከ`iroha app da prove-availability` ጋር
   እና የ PDP ማስረጃ.

የተሃድሶ ሽፋን በ `integration_tests/tests/da/replication_policy.rs` ውስጥ ይኖራል;
ክፍሉ ያልተዛመደ የማቆያ ፖሊሲ ለ`/v2/da/ingest` ያቀርባል እና ያረጋግጣል
የመጣው ማኒፌክት ከደዋዩ ይልቅ የግዳጅ መገለጫውን እንደሚያጋልጥ
ዓላማ

## የጤና ማረጋገጫ ቴሌሜትሪ እና ዳሽቦርዶች (DA-5 ድልድይ)

የፍኖተ ካርታ ንጥል **DA-5** የ PDP/PoTR ማስፈጸሚያ ውጤቶች ኦዲት እንዲደረጉ ይጠይቃል
እውነተኛ ጊዜ. `SorafsProofHealthAlert` ክስተቶች አሁን የተወሰነ ስብስብ ያንቀሳቅሳሉ
Prometheus ሜትሪክስ፡

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

የ **SoraFS ፒፒዲ እና ፖትአር ጤና** Grafana ሰሌዳ
(`dashboards/grafana/sorafs_pdp_potr_health.json`) አሁን እነዚህን ምልክቶች ያጋልጣል፡-- *የጤና ማንቂያዎችን በማስቀስቀስ ማረጋገጫ* ገበታዎች የማንቂያ ታሪፎችን በመቀስቀስ/በቅጣት ባንዲራ እንዲሁ
  የታይካይ/ሲዲኤን ኦፕሬተሮች ፒዲፒ-ብቻ፣ፖTR-ብቻ ወይም ድርብ ምልክቶች መሆናቸውን ማረጋገጥ ይችላሉ።
  መተኮስ።
- *Cooldown ውስጥ ያሉ አቅራቢዎች* በአሁኑ ጊዜ የአቅራቢዎችን የቀጥታ ድምር ሪፖርት በ ሀ
  SorafsProofHealthAlert ማቀዝቀዝ።
- *የጤና ማረጋገጫ መስኮት ቅጽበታዊ ገጽ እይታ* የ PDP/PoTR ቆጣሪዎችን፣ የቅጣት መጠን፣
  የማቀዝቀዝ ባንዲራ፣ እና የአድማ መስኮት ፍጻሜ ዘመን በአገልግሎት አቅራቢ ስለዚህ የአስተዳደር ገምጋሚዎች
  ሰንጠረዡን ከአደጋ እሽጎች ጋር ማያያዝ ይችላል.

የDA ማስፈጸሚያ ማስረጃዎችን ሲያቀርቡ Runbooks እነዚህን ፓነሎች ማገናኘት አለባቸው። እነሱ
የCLI የማረጋገጫ ዥረት ውድቀቶችን በቀጥታ በሰንሰለት ላይ ካለው የቅጣት ዲበ ውሂብ ጋር ማሰር እና
በፍኖተ ካርታው ላይ የተጠራውን ታዛቢነት መንጠቆ ያቅርቡ።