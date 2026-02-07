---
lang: am
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ርዕስ: የውሂብ ተገኝነት ማባዛት ፖሊሲ
sidebar_label፡ የማባዛት ፖሊሲ
መግለጫ፡- በመንግስት የተተገበሩ የማቆያ መገለጫዎች በሁሉም የዲኤ ማስገባቶች ላይ ተተግብረዋል።
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

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
ሁሉም `/v1/da/ingest` ማቅረቢያዎች. Torii እንደገና ይጽፋል ከግዳጅ ጋር
ፕሮፋይሉን ማቆየት እና ደዋዮች የማይዛመዱ እሴቶችን ሲያቀርቡ ማስጠንቀቂያ ይሰጣል
ኦፕሬተሮች ያረጁ ኤስዲኬዎችን ማወቅ ይችላሉ።

### የታይካይ ተደራሽነት ክፍሎች

የታይካይ ማዘዋወር መግለጫዎች (`taikai.trm`) `availability_class` አውጀዋል
(`hot`፣ `warm`፣ ወይም `cold`)። Torii ከመቁረጥ በፊት የማዛመጃ ፖሊሲን ያስፈጽማል
ስለዚህ ኦፕሬተሮች ዓለም አቀፉን ሳያርትዑ በአንድ ዥረት የተባዙ ቆጠራዎችን ማመጣጠን ይችላሉ።
ጠረጴዛ. ነባሪዎች፡-

| ተገኝነት ክፍል | ትኩስ ማቆየት | ቀዝቃዛ ማቆየት | የሚፈለጉ ቅጂዎች | የማከማቻ ክፍል | አስተዳደር መለያ |
|------------------
| `hot` | 24 ሰአት | 14 ቀናት | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ሰአት | 30 ቀናት | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ሰአት | 180 ቀናት | 3 | `cold` | `da.taikai.archive` |

የጎደሉ ፍንጮች ለI18NI0000042X ነባሪ ስለዚህ የቀጥታ ስርጭቶች በጣም ጠንካራውን ፖሊሲ ይዘው ይቆያሉ።
ነባሪዎችን በ በኩል ይሽሩ
አውታረ መረብዎ የሚጠቀም ከሆነ `torii.da_ingest.replication_policy.taikai_availability`
የተለያዩ ዒላማዎች.

## ማዋቀር

ፖሊሲው የሚኖረው በI18NI0000044X ሲሆን አጋልጧል
*ነባሪ* አብነት እና የክፍል መሻሮች ድርድር። የክፍል መለያዎች ናቸው።
ለጉዳይ የማይሰማ እና `taikai_segment`፣ `nexus_lane_sidecar` ተቀበል፣
`governance_artifact`፣ ወይም `custom:<u16>` ለአስተዳደር-የጸደቁ ቅጥያዎች።
የማከማቻ ክፍሎች I18NI0000049X፣ `warm`፣ ወይም `cold` ይቀበላሉ።

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
`default_retention` አርትዕ.

የታይካይ ተገኝነት ክፍሎች በተናጥል ሊሻሩ ይችላሉ
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## ማስፈጸሚያ ትርጉሞች

- Torii በተጠቃሚ የቀረበውን `RetentionPolicy` በተተገበረው መገለጫ ይተካዋል
  ልቀትን ከመቁረጥ ወይም ከማሳየት በፊት።
- ያልተዛመደ የማቆያ ፕሮፋይል ውድቅ መደረጉን የሚገልጹ ቀድሞ-የተገነቡ አንጸባራቂዎች
  ከ `400 schema mismatch` ጋር የቆዩ ደንበኞች ውሉን ማዳከም አይችሉም።
- እያንዳንዱ የመሻር ክስተት ተመዝግቧል (`blob_class`፣ ከተጠበቀው ፖሊሲ ጋር ገብቷል)
  በታቀደው ጊዜ ታዛዥ ያልሆኑ ደዋዮችን ለማሳየት።

ለተሻሻለው በር [የመረጃ ተገኝነት የኢንጀስት እቅድ](ingest-plan.md) (የማረጋገጫ ዝርዝር) ይመልከቱ።
የማቆያ አፈፃፀምን የሚሸፍን.

## እንደገና ማባዛት የስራ ፍሰት (DA-4 ክትትል)

የማቆየት አፈፃፀም የመጀመሪያው እርምጃ ብቻ ነው። ኦፕሬተሮችም ያንን ማረጋገጥ አለባቸው
የቀጥታ መግለጫዎች እና የማባዛት ትዕዛዞች ከተዋቀረው መመሪያ ጋር አብረው ይቆያሉ።
SoraFS ከታዛዥነት ውጪ የሆኑ ነጠብጣቦችን በራስ ሰር እንደገና ሊደግም ይችላል።

1. ** ለመንሸራተት ተመልከት።** Torii የሚለቀቀው
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

   ትዕዛዙ I18NI0000059X ከተሰጠው ይጭናል
   ማዋቀር፣ እያንዳንዱን አንጸባራቂ (JSON ወይም Norito) መፍታት እና እንደ አማራጭ ከማንኛውም ጋር ይዛመዳል።
   `ReplicationOrderV1` የሚጫኑ ጭነቶች በአንፀባራቂ መፍጨት። ማጠቃለያው ሁለት ባንዲራ ነው።
   ሁኔታዎች፡-

   - `policy_mismatch` - አንጸባራቂ ማቆየት መገለጫ ከተገደደው ይለያል
     ፖሊሲ (Torii በተሳሳተ መንገድ ካልተዋቀረ በስተቀር ይህ በፍፁም መከሰት የለበትም)።
   - `replica_shortfall` - የቀጥታ ማባዛት ትዕዛዝ ያነሰ ቅጂዎችን ይጠይቃል
     `RetentionPolicy.required_replicas` ወይም ከእሱ ያነሱ ስራዎችን ያቀርባል
     ዒላማ.

   ዜሮ ያልሆነ የመውጫ ሁኔታ የ CI/የጥሪ አውቶማቲክ ገባሪ ጉድለትን ያሳያል
   ወዲያውኑ ገጽ ማድረግ ይችላል። የJSON ዘገባውን ከ
   `docs/examples/da_manifest_review_template.md`
   ለፓርላማ ድምጾች ፓኬት.
3. ** ድጋሚ ማባዛትን ቀስቅሰው።** ኦዲቱ ጉድለት እንዳለ ሲዘግብ አዲስ ያወጣል።
   `ReplicationOrderV1` በተገለጸው የአስተዳደር መሣሪያ በኩል
   [SoraFS የማጠራቀሚያ አቅም ገበያ ቦታ](../sorafs/storage-capacity-marketplace.md) እና ኦዲቱን እንደገና ያስጀምሩ
   የተባዛው ስብስብ እስኪቀላቀል ድረስ. ለአደጋ ጊዜ መሻር፣ የCLI ውፅዓት ያጣምሩ
   SREs ተመሳሳዩን መፈጨት ማጣቀስ እንዲችሉ በ`iroha app da prove-availability`
   እና የ PDP ማስረጃ.

የተሃድሶ ሽፋን በ I18NI0000067X ውስጥ ይኖራል;
ክፍሉ ያልተዛመደ የማቆያ ፖሊሲ ለ`/v1/da/ingest` ያቀርባል እና ያረጋግጣል
የመጣው ማኒፌክት ከደዋዩ ይልቅ የግዳጅ መገለጫውን እንደሚያጋልጥ
ዓላማ