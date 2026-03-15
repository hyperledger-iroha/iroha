---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cc63e7549adebfe3ab539eca608e2fc88830361b3fe53b165491e36ecb83177
source_last_modified: "2026-01-22T14:35:36.748626+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-plan
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS ፒን መዝገብ ቤት ትግበራ እቅድ (SF-4)

SF-4 የፒን መዝገብ ቤት ውል እና የሚያከማቹ ደጋፊ አገልግሎቶችን ያቀርባል
ቃል ኪዳኖችን ማሳየት፣ የፒን ፖሊሲዎችን ማስፈጸም እና ኤፒአይዎችን ለTorii፣ መግቢያ መንገዶች፣
እና ኦርኬስትራዎች. ይህ ሰነድ የማረጋገጫ እቅድን በኮንክሪት ያሰፋዋል
የመተግበር ተግባራት፣ በሰንሰለት ላይ ያለውን አመክንዮ የሚሸፍን ፣ የአስተናጋጅ-ጎን አገልግሎቶች ፣ የቤት ዕቃዎች ፣
እና የአሠራር መስፈርቶች.

## ወሰን

1. ** የመመዝገቢያ ግዛት ማሽን ***: Norito-የተገለጹ መዝገቦች ለገለጻዎች, ተለዋጭ ስሞች,
   ተተኪ ሰንሰለቶች፣ የማቆየት ዘመን እና የአስተዳደር ዲበ ውሂብ።
2. **የኮንትራት ትግበራ**፡ ለፒን የህይወት ኡደት የሚወስኑ የCRUD ስራዎች
   (`ReplicationOrder`፣ `Precommit`፣ `Completion`፣ ማስወጣት)።
3. **የአገልግሎት ፊት ለፊት**፡ gRPC/REST በ Torii በመዝገቡ የተደገፈ የመጨረሻ ነጥቦች
   እና ኤስዲኬዎች በገጽ መግለጫ እና ማረጋገጫን ጨምሮ ይበላሉ።
4. ** መሳሪያዎች እና እቃዎች ***: CLI አጋዥዎች፣ ቬክተሮችን ይፈትሹ እና የሚቀመጡ ሰነዶች
   ይገለጻል፣ ተለዋጭ ስሞች እና የአስተዳደር ኤንቨሎፖች በማመሳሰል።
5. **ቴሌሜትሪ እና ኦፕስ**፡ መለኪያዎች፣ ማንቂያዎች እና የሩጫ መጽሐፍት ለመመዝገቢያ ጤና።

## የውሂብ ሞዴል

### ኮር መዛግብት (Norito)

| መዋቅር | መግለጫ | መስኮች |
|--------|-------------|----|
| `PinRecordV1` | ቀኖናዊ አንጸባራቂ መግቢያ። | `manifest_cid`፣ `chunk_plan_digest`፣ `por_root`፣ `profile_handle`፣ `approved_at`፣ `retention_epoch`፣ I18NI00000031000 `governance_envelope_hash`. |
| `AliasBindingV1` | ካርታዎች ተለዋጭ ስም -> አንጸባራቂ CID። | `alias`፣ `manifest_cid`፣ `bound_at`፣ `expiry_epoch`። |
| `ReplicationOrderV1` | አንጸባራቂን ለመሰካት አቅራቢዎች መመሪያ። | `order_id`፣ `manifest_cid`፣ `providers`፣ `redundancy`፣ `deadline`፣ `policy_hash`። |
| `ReplicationReceiptV1` | የአቅራቢ እውቅና. | `order_id`፣ `provider_id`፣ `status`፣ `timestamp`፣ `por_sample_digest`። |
| `ManifestPolicyV1` | የአስተዳደር ፖሊሲ ቅጽበታዊ ገጽ እይታ። | `min_replicas`፣ `max_retention_epochs`፣ `allowed_profiles`፣ `pin_fee_basis_points`። |

የትግበራ ማጣቀሻ፡ ለ `crates/sorafs_manifest/src/pin_registry.rs` ይመልከቱ
Rust Norito እቅዶች እና የማረጋገጫ ረዳቶች እነዚህን መዝገቦች ይደግፋሉ። ማረጋገጫ
አንጸባራቂውን መሳሪያ (chunker registry lookup፣ pin policy gating) ያንጸባርቃል
ውል፣ Torii የፊት ገጽታዎች፣ እና CLI ተመሳሳይ ልዩነቶችን ይጋራሉ።

ተግባራት፡
- በ `crates/sorafs_manifest/src/pin_registry.rs` ውስጥ የ Norito መርሃግብሮችን ያጠናቅቁ።
- Norito ማክሮዎችን በመጠቀም ኮድ (ዝገት + ሌሎች ኤስዲኬዎች) ይፍጠሩ።
- ሰነዶችን ያዘምኑ (`sorafs_architecture_rfc.md`) አንድ ጊዜ እቅድ ካወጣ።

## የውል አፈፃፀም

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| የመመዝገቢያ ማከማቻ (sled/sqlite/off-chain) ወይም ስማርት ኮንትራት ሞጁሉን ተግብር። | ኮር ኢንፍራ / ስማርት ኮንትራት ቡድን | የሚወስን hashing ያቅርቡ፣ ተንሳፋፊ ነጥብ ያስወግዱ። |
| የመግቢያ ነጥቦች፡- `submit_manifest`፣ `approve_manifest`፣ `bind_alias`፣ `issue_replication_order`፣ `complete_replication`፣ `evict_manifest`። | ኮር ኢንፍራ | ከማረጋገጫ እቅድ `ManifestValidator` ይጠቀሙ። ተለዋጭ ስም ማሰር አሁን በ`RegisterPinManifest` (Torii DTO surfacing) በኩል ይፈስሳል ፣የተሰጠ `bind_alias` ለተከታታይ ዝመናዎች የታቀደ ሆኖ ይቆያል። |
| የግዛት ሽግግሮች፡- ተተኪነትን ማስፈጸም (ገለጻው A -> B)፣ የማቆየት ዘመናት፣ ልዩ ተለዋጭ ስም። | የአስተዳደር ምክር ቤት / ኮር ኢንፍራ | ልዩ ተለዋጭ ስም፣ የማቆየት ገደቦች እና የቀደመው ማፅደቅ/የጡረታ ቼኮች አሁን በ`crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ውስጥ ይኖራሉ። የብዝሃ-ሆፕ ተከታይ ማግኘት እና ማባዛት የሂሳብ አያያዝ ክፍት እንደሆኑ ይቆያሉ። |
| የሚተዳደሩ መለኪያዎች: ጭነት I18NI0000070X ከ ውቅር / አስተዳደር ሁኔታ; በአስተዳደር ክስተቶች በኩል ዝማኔዎችን ፍቀድ። | አስተዳደር ምክር ቤት | ለፖሊሲ ማሻሻያ CLI ያቅርቡ። |
| የክስተት ልቀት፡ ለቴሌሜትሪ (`ManifestApproved`፣ I18NI0000072X፣ `AliasBound`) I18NT0000008X ክስተቶችን ልቀቁ። | ታዛቢነት | የክስተት ንድፍ + ምዝግብ ማስታወሻን ይግለጹ። |

በመሞከር ላይ፡
- ለእያንዳንዱ የመግቢያ ነጥብ (አዎንታዊ + ውድቅ) የክፍል ሙከራዎች።
- ለተከታታይ ሰንሰለት የንብረት ሙከራዎች (ሳይክሎች የሉም ፣ ሞኖቶኒክ ኢፖኮች)።
- የዘፈቀደ መገለጫዎችን በማመንጨት Fuzz ማረጋገጥ (የተገደበ)።

## የአገልግሎት ፊት (Torii/ኤስዲኬ ውህደት)

| አካል | ተግባር | ባለቤት(ዎች) |
|--------|------|------|
| Torii አገልግሎት | `/v2/sorafs/pin` (አስገባ)፣ `/v2/sorafs/pin/{cid}` (መፈለግ)፣ `/v2/sorafs/aliases` (ዝርዝር/ማሰር)፣ `/v2/sorafs/replication` (ትዕዛዞች/ደረሰኞች) አጋልጥ። ፔጅኔሽን + ማጣሪያ ያቅርቡ። | አውታረ መረብ TL / ኮር ኢንፍራ |
| ምስክርነት | በምላሾች ውስጥ የመመዝገቢያ ቁመት / hash ያካትቱ; በኤስዲኬዎች የሚበላ የNorito ማረጋገጫ መዋቅር ያክሉ። | ኮር ኢንፍራ |
| CLI | `sorafs_manifest_stub` ወይም አዲስ I18NI0000079X CLI በ I18NI0000080X፣ `alias bind`፣ `order issue`፣ `registry export` ያራዝም። | Tooling WG |
| ኤስዲኬ | የደንበኛ ማሰሪያዎችን (Rust/Go/TS) ከ Norito schema መፍጠር; የውህደት ሙከራዎችን ይጨምሩ. | የኤስዲኬ ቡድኖች |

ተግባራት፡-
- ለGET የመጨረሻ ነጥቦች የመሸጎጫ ንብርብር/ETag ይጨምሩ።
- ከTorii ፖሊሲዎች ጋር የሚስማማ የዋጋ መገደብ/ማሳመን ያቅርቡ።

## ቋሚዎች እና CI

- የቋሚዎች ማውጫ፡- `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` መደብሮች የተፈረሙ የማኒፌክት/ተለዋጭ ስም/በ`cargo run -p iroha_core --example gen_pin_snapshot` የታደሱ ቅጽበታዊ ገጽ እይታዎች።
- CI ደረጃ፡- I18NI0000086X ቅጽበተ-ፎቶውን ያድሳል እና ልዩነቶች ከታዩ አይሳካም ፣የ CI መጫዎቻዎች የተስተካከሉ እንዲሆኑ ያደርጋል።
- የውህደት ፈተናዎች (`crates/iroha_core/tests/pin_registry.rs`) የደስታ መንገድን እና የተባዛ-ተለዋጭ ስም አለመቀበልን፣ ቅጽል ስም ማፅደቅ/ማቆያ ጠባቂዎች፣ ያልተዛመደ ሹንከር እጀታዎች፣ ቅጂ-ቆጠራ ማረጋገጥ እና ተተኪ-ጠባቂ ውድቀቶች (ያልታወቀ/ቅድመ-ፀደቀ/ጡረታ/ራስ ጠቋሚዎች)። ለሽፋን ዝርዝሮች `register_manifest_rejects_*` ጉዳዮችን ይመልከቱ።
- የዩኒት ሙከራዎች አሁን በ `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ውስጥ ተለዋጭ ማረጋገጫን ፣ ማቆያ ጠባቂዎችን እና ተተኪ ቼኮችን ይሸፍናሉ ። ባለብዙ ሆፕ ተከታይ ማወቂያ አንዴ የመንግስት ማሽን መሬት።
- ወርቃማው JSON በተመልካችነት ቧንቧዎች ለሚጠቀሙባቸው ዝግጅቶች።

## ቴሌሜትሪ እና ታዛቢነት

መለኪያዎች (Prometheus)
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- ነባር አቅራቢ ቴሌሜትሪ (I18NI0000096X፣ `torii_sorafs_fee_projection_nanos`) ከጫፍ እስከ ጫፍ ዳሽቦርዶች ወሰን ውስጥ ይቆያል።

መዝገቦች፡
- የተዋቀረ I18NT0000011X የክስተት ዥረት ለአስተዳደር ኦዲቶች (የተፈረመ?)።

ማንቂያዎች፡
- ከ SLA በላይ የሆኑ የማባዛት ትዕዛዞች በመጠባበቅ ላይ።
- ተለዋጭ ስም ጊዜው ያበቃል < ደፍ።
- የማቆየት ጥሰቶች (ማለቂያው ከማለቁ በፊት ያልታደሰ መግለጫ)።

ዳሽቦርዶች፡
- Grafana JSON I18NI0000098X ትራኮች የህይወት ኡደት ድምርን፣ተለዋጭ ሽፋን፣የኋላ ሎግ ሙሌት፣ SLA ሬሾ፣የዘገየ እና የላላ ተደራቢዎች፣እና ለጥሪ ግምገማ ያመለጡ የትዕዛዝ መጠኖች።

## Runbooks & Documentation

- የመመዝገቢያ ሁኔታ ዝመናዎችን ለማካተት I18NI00000099ን ያዘምኑ።
- የኦፕሬተር መመሪያ፡ `docs/source/sorafs/runbooks/pin_registry_ops.md` (አሁን የታተመ) መለኪያዎችን፣ ማንቂያዎችን፣ ማሰማራትን፣ ምትኬን እና የመልሶ ማግኛ ፍሰቶችን የሚሸፍን ነው።
- የአስተዳደር መመሪያ፡ የፖሊሲ መለኪያዎችን ይግለጹ፣ የተፈቀደ የስራ ሂደት፣ የክርክር አያያዝ።
- ለእያንዳንዱ የመጨረሻ ነጥብ (Docusaurus ሰነዶች) የኤፒአይ ማመሳከሪያ ገጾች።

## ጥገኛ እና ቅደም ተከተል

1. የተሟላ የማረጋገጫ እቅድ ተግባራት (ManiifestValidator ውህደት).
2. Norito schema + የፖሊሲ ነባሪዎችን ያጠናቅቁ።
3. ውል + አገልግሎት, ሽቦ ቴሌሜትሪ ተግባራዊ ያድርጉ.
4. የቤት እቃዎችን እንደገና ማደስ, የመዋሃድ ስብስቦችን ያሂዱ.
5. ሰነዶች/ runbooks ያዘምኑ እና የመንገድ ካርታ እቃዎች እንደተጠናቀቁ ምልክት ያድርጉ።

በSF-4 ስር ያለው እያንዳንዱ የፍኖተ ካርታ ማረጋገጫ ዝርዝር ይህ እቅድ መሻሻል ሲደረግ ማጣቀስ አለበት።
የREST የፊት ገጽታ አሁን የተረጋገጡ የዝርዝር የመጨረሻ ነጥቦችን ይላካል፡

- `GET /v2/sorafs/pin` እና `GET /v2/sorafs/pin/{digest}` መመለስ በ
  ተለዋጭ ስም ማሰር፣ የማባዛት ትዕዛዞች እና ከ የተገኘ የማረጋገጫ ነገር
  የቅርብ ጊዜ እገዳ ሃሽ.
- `GET /v2/sorafs/aliases` እና `GET /v2/sorafs/replication` ንቁውን ያጋልጣሉ
  ቅጽል ካታሎግ እና የማባዛት ቅደም ተከተል የኋላ ሎግ ወጥነት ባለው ገጽ እና
  የሁኔታ ማጣሪያዎች.

CLI እነዚህን ጥሪዎች ያጠቃልላል (`iroha app sorafs pin list`፣ `pin show`፣ `alias list`፣
`replication list`) ስለዚህ ኦፕሬተሮች የመዝገብ ኦዲቶችን ሳይነኩ መፃፍ ይችላሉ
ዝቅተኛ ደረጃ ኤፒአይዎች።