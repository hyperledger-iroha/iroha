---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 868edd6aa7401c64b8757db188edb13aa8e6ca8959966b6fea02e44bc298c6b7
source_last_modified: "2026-01-05T09:28:11.910794+00:00"
translation_last_reviewed: 2026-02-07
id: storage-capacity-marketplace
title: SoraFS Storage Capacity Marketplace
sidebar_label: Capacity Marketplace
description: SF-2c plan for the capacity marketplace, replication orders, telemetry, and governance hooks.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS የማከማቻ አቅም የገበያ ቦታ (SF-2c ረቂቅ)

የ SF-2c የመንገድ ካርታ ንጥል ማከማቻ ቦታ የሚተዳደር የገበያ ቦታን ያስተዋውቃል
አቅራቢዎች ቁርጠኛ አቅምን ያውጃሉ፣ የማባዛት ትዕዛዞችን ይቀበላሉ እና ክፍያዎችን ያገኛሉ
ከቀረበው አቅርቦት ጋር ተመጣጣኝ። ይህ ሰነድ የመላኪያዎችን ወሰን ያጠቃልላል
ለመጀመሪያው ልቀት ያስፈልጋል እና ወደ ተግባራዊ ትራኮች ይሰብሯቸዋል።

# አላማዎች

- የአቅራቢዎችን አቅም ቁርጠኝነት ይግለጹ (ጠቅላላ ባይት ፣ የሌይን ገደቦች ፣ የአገልግሎት ማብቂያ ጊዜ)
  በአስተዳደር፣ በሶራኔት ትራንስፖርት እና በTorii ሊገለገል በሚችል የተረጋገጠ ቅጽ።
- በተገለጸው አቅም፣ ድርሻ እና በአቅራቢዎች ላይ ፒኖችን መድብ
  የመወሰን ባህሪን በመጠበቅ ላይ የፖሊሲ ገደቦች።
- ሜትር ማከማቻ ማድረስ (የማባዛት ስኬት፣ የስራ ሰዓት፣ የአቋም ማረጋገጫዎች) እና
  ለክፍያ ማከፋፈያ የመላክ ቴሌሜትሪ.
- ሐቀኝነት የጎደላቸው አቅራቢዎች እንዲሆኑ የስረዛ እና የክርክር ሂደቶችን ያቅርቡ
  ተቀጥቷል ወይም ተወግዷል.

## የጎራ ጽንሰ-ሀሳቦች

| ጽንሰ | መግለጫ | መጀመሪያ ሊደርስ የሚችል |
|--------|-------------|
| `CapacityDeclarationV1` | Norito የክፍያ ጭነት የአቅራቢ መታወቂያን፣ ቸንከር ፕሮፋይል ድጋፍን፣ ቁርጠኛ ጂቢን፣ ሌይን-ተኮር ገደቦችን፣ የዋጋ አወጣጥ ፍንጮችን፣ ቁርጠኝነትን እና የአገልግሎት ማብቂያ ጊዜን የሚገልጽ። | Schema + አረጋጋጭ በI18NI0000029X። |
| `ReplicationOrder` | የድጋሚነት ደረጃን እና SLA መለኪያዎችን ጨምሮ አንጸባራቂ CID ለአንድ ወይም ከዚያ በላይ አቅራቢዎች የሚመደብ በመንግስት የተሰጠ መመሪያ። | Norito እቅድ ከI18NT0000017X + smart contract API ጋር ተጋርቷል። |
| `CapacityLedger` | በሰንሰለት ላይ/ከሰንሰለት ውጪ የነቃ የአቅም መግለጫዎችን፣የማባዛት ትዕዛዞችን፣የአፈጻጸም መለኪያዎችን እና የክፍያ ማጠራቀምን መከታተል። | ብልጥ የኮንትራት ሞጁል ወይም ከሰንሰለት ውጪ የአገልግሎት ግትር ከወሳኝ ቅጽበታዊ ፎቶ ጋር። |
| `MarketplacePolicy` | የአስተዳደር ፖሊሲ ዝቅተኛውን ድርሻ፣ የኦዲት መስፈርቶችን እና የቅጣት ኩርባዎችን ይገልጻል። | በI18NI0000033X + የአስተዳደር ሰነድ ውስጥ መዋቅርን ያዋቅሩ። |

### የተተገበሩ እቅዶች (ሁኔታ)

##የስራ መፈራረስ

### 1. Schema & Registry Layer

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| `CapacityDeclarationV1`፣ `ReplicationOrderV1`፣ `CapacityTelemetryV1` ይግለጹ። | የማከማቻ ቡድን / አስተዳደር | Norito ይጠቀሙ; የትርጉም እትም እና የችሎታ ማጣቀሻዎችን ያካትቱ። |
| ተንታኝ + አረጋጋጭ ሞጁሎችን በI18NI0000037X ውስጥ ተግብር። | የማከማቻ ቡድን | ነጠላ መታወቂያዎችን፣ የአቅም ገደቦችን፣ የካስማ መስፈርቶችን ያስፈጽሙ። |
| በ`min_capacity_gib` በመገለጫ chunker መዝገብ ቤት ሜታዳታን ያራዝሙ። | Tooling WG | ደንበኞች በየመገለጫ ቢያንስ የሃርድዌር መስፈርቶችን እንዲያስፈጽሙ ያግዛል። |
| ረቂቅ `MarketplacePolicy` ሰነድ የመግቢያ መከላከያ መንገዶችን እና የቅጣት መርሃ ግብርን የሚይዝ። | አስተዳደር ምክር ቤት | ከመመሪያ ነባሪዎች ጋር በሰነዶች ያትሙ። |

#### የመርሃግብር ፍቺዎች (ተግባራዊ)

- `CapacityDeclarationV1` ቀኖናዊ chunker እጀታዎች፣ የችሎታ ማጣቀሻዎች፣ የአማራጭ ሌይን ካፕ፣ የዋጋ አወጣጥ ፍንጮች፣ ተቀባይነት ያላቸው መስኮቶች እና ሜታዳታ ጨምሮ በአንድ አቅራቢ የተፈረሙ የአቅም ቃላቶችን ይይዛል። ማረጋገጫው ዜሮ ያልሆኑ አክሲዮኖችን፣ ቀኖናዊ እጀታዎችን፣ የተባዙ ተለዋጭ ስሞችን፣ በሌይን ኮፍያዎችን በታወጀው ጠቅላላ ድምር እና ነጠላ የጂቢ ሂሳብ አያያዝን ያረጋግጣል።【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` በአስተዳደር የተሰጡ ስራዎችን ከቅጣት ዒላማዎች፣ የኤስኤ ገደቦች እና በእያንዳንዱ ምድብ ዋስትናዎች ጋር ይያያዛል። አረጋጋጮች ቀኖናዊ ቻንከር እጀታዎችን፣ ልዩ አቅራቢዎችን እና የግዜ ገደብ ገደቦችን ከTorii በፊት ያስፈጽማሉ ወይም መዝገቡ ትዕዛዙን ከመግባቱ በፊት።【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` የክፍያ ስርጭትን የሚመገቡ የኤፒኮ ቅጽበተ-ፎቶዎችን (በጥቅም ላይ የዋለው ጂቢ፣ የማባዛት ቆጣሪዎች፣ የሰአት/PoR መቶኛ) ይገልጻል። የድንበር ቼኮች በአወጆች እና በመቶኛዎች በ0 - 100% ውስጥ አጠቃቀሙን ያቆያሉ【crates/sorafs_manifest/src/capacity.rs:476】
- የተጋሩ ረዳቶች (`CapacityMetadataEntry`፣ `PricingScheduleV1`፣ ሌይን/መመደብ/SLA አረጋጋጮች) CI እና ታችኛው ተፋሰስ መገልገያ እንደገና ጥቅም ላይ ሊውሉ እንደሚችሉ የሚወስን ቁልፍ ማረጋገጫ እና የስህተት ሪፖርት ያቀርባሉ።【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` አሁን በሰንሰለት ላይ ያለውን ቅጽበታዊ ገጽ እይታ በ`/v2/sorafs/capacity/state` በኩል ያቀርባል፣ የአቅራቢዎችን መግለጫዎች እና የክፍያ ደብተር ግቤቶችን ከወሳኙ I18NT0000008X ጋር በማጣመር ጄሰን።
- የማረጋገጫ ሽፋን ልምምዶች ቀኖናዊ እጀታ ማስፈጸሚያ፣ የተባዛ ፈልጎ ማግኘት፣ በየመንገድ ድንበሮች፣ መባዛት ጥበቃዎች እና የቴሌሜትሪ ክልል ፍተሻዎች ወዲያውኑ በCI ውስጥ እንደገና እንዲታዩ ያደርጋል።【crates/sorafs_manifest/src/capacity.rs:792】
- ኦፕሬተር መሳሪያ፡ `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` በሰው ሊነበቡ የሚችሉ ዝርዝሮችን ወደ ቀኖናዊ Norito ክፍያ ጭነቶች፣ ቤዝ64 ብሎብስ እና JSON ማጠቃለያዎች ስለሚለውጥ ኦፕሬተሮች የ `/v2/sorafs/capacity/declare`፣ `/v2/sorafs/capacity/telemetry`፣ `/v2/sorafs/capacity/telemetry`ን ደረጃ እንዲይዙ እና ከአካባቢያዊ ትግበራ ጋር። validation.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 የማጣቀሻ እቃዎች በ`fixtures/sorafs_manifest/replication_order/` (`order_v1.json`፣ `order_v1.json`፣ `order_v1.to`) እና በI.08NI050501

### 2. የመቆጣጠሪያ አውሮፕላን ውህደት

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| `/v2/sorafs/capacity/declare`፣ `/v2/sorafs/capacity/telemetry`፣ `/v2/sorafs/capacity/orders` Torii ተቆጣጣሪዎችን ከNorito JSON ጭነት ጋር ይጨምሩ። | Torii ቡድን | የመስታወት አረጋጋጭ አመክንዮ; Norito JSON አጋዥዎችን እንደገና ተጠቀም። |
| `CapacityDeclarationV1` ቅጽበተ-ፎቶዎችን ወደ ኦርኬስትራ የውጤት ሰሌዳ ሜታዳታ እና የጌትዌይ ማምጣት እቅዶችን ያሰራጩ። | Tooling WG / ኦርኬስትራ ቡድን | የብዝሃ-ምንጭ ነጥብ የሌይን ወሰኖችን ያከብራል። |
| የተሰጡ ስራዎችን እና ያልተሳኩ ፍንጮችን ለመንዳት የማባዛት ትዕዛዞችን ወደ ኦርኬስትራ/ጌትዌይ ደንበኞች ይመግቡ። | አውታረ መረብ TL / ጌትዌይ ቡድን | የውጤት ሰሌዳ ገንቢ በአስተዳደር የተፈረሙ የማባዛት ትዕዛዞችን ይበላል። |
| የ CLI መሳሪያ ስራ፡ `capacity declare`፣ `capacity telemetry`፣ `capacity orders import` ጋር I18NI0000059X ዘርጋ። | Tooling WG | የሚወስን JSON + የውጤት ሰሌዳ ውጤቶችን ያቅርቡ። |

### 3. የገበያ ቦታ ፖሊሲ እና አስተዳደር

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| `MarketplacePolicy` (ዝቅተኛውን ድርሻ፣ የቅጣት ማባዣዎች፣ የኦዲት ማረጋገጫ) አጽድቅ። | አስተዳደር ምክር ቤት | በሰነዶች ውስጥ ያትሙ፣ የክለሳ ታሪክን ይያዙ። |
| ፓርላማ ማጽደቅ፣ ማደስ እና መግለጫዎችን መሻር እንዲችል የአስተዳደር መንጠቆዎችን ያክሉ። | የአስተዳደር ምክር ቤት / ስማርት ኮንትራት ቡድን | የNorito ክስተቶችን + አንጸባራቂ መግቢያን ተጠቀም። |
| ከቴሌሜትር SLA ጥሰቶች ጋር የተሳሰረ የቅጣት መርሃ ግብር (የክፍያ ቅነሳ፣ የቦንድ ቅነሳ) ተግብር። | የአስተዳደር ምክር ቤት / ግምጃ ቤት | ከ`DealEngine` የሰፈራ ውጤቶች ጋር አሰልፍ። |
| የሰነድ ሙግት ሂደት እና የመጨመር ማትሪክስ። | ሰነዶች / አስተዳደር | ወደ ክርክር runbook + CLI አጋዥዎች አገናኝ። |

### 4. የመለኪያ እና ክፍያ ስርጭት

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| `CapacityTelemetryV1` ለመቀበል Torii የመለኪያ ውስጠትን ዘርጋ። | Torii ቡድን | GiB-ሰዓቶችን፣ የPoR ስኬትን፣ የስራ ጊዜን ያረጋግጡ። |
| በየትዕዛዝ አጠቃቀም + SLA ስታቲስቲክስ ሪፖርት ለማድረግ `sorafs_node` የመለኪያ ቧንቧ ያዘምኑ። | የማከማቻ ቡድን | ከማባዛት ትዕዛዞች እና ቺንከር እጀታዎች ጋር አሰልፍ። |
| የመቋቋሚያ ቧንቧ፡ የቴሌሜትሪ + የማባዛት መረጃን ወደ XOR-የተከፋፈሉ ክፍያዎች መለወጥ፣ ለአስተዳደር ዝግጁ የሆኑ ማጠቃለያዎችን ማዘጋጀት እና የመመዝገቢያ ሁኔታን መመዝገብ። | ግምጃ ቤት / ማከማቻ ቡድን | ሽቦ ወደ Deal Engine / የግምጃ ቤት ወደ ውጭ መላክ። |
| ጤናን ለመለካት ዳሽቦርዶችን/ ማንቂያዎችን ወደ ውጭ ይላኩ (የማስገባት ጀርባ ፣ የቆየ ቴሌሜትሪ)። | ታዛቢነት | በSF-6/SF-7 የተጠቀሰውን የGrafana ጥቅል ያራዝሙ። |

- Torii አሁን I18NI0000067X እና I18NI0000068X (JSON + Norito) ያጋልጣል ስለዚህ ኦፕሬተሮች የኤፖክ ቴሌሜትሪ ቅጽበተ ፎቶዎችን እንዲያቀርቡ እና ተቆጣጣሪዎች ለኦዲት ወይም ለማስረጃ ቀኖናዊ ደብተር ማውጣት ይችላሉ። ማሸግ።
- `PinProviderRegistry` ውህደት የማባዛት ትዕዛዞች በተመሳሳዩ የመጨረሻ ነጥብ ተደራሽ መሆናቸውን ያረጋግጣል። የCLI አጋዥዎች (`sorafs_cli capacity telemetry --from-file telemetry.json`) አሁን ቴሌሜትሪዎችን ከአውቶሜሽን በሚወስነው ሃሺንግ እና ተለዋጭ መፍታት አረጋግጠዋል/ አትመዋል።
- የመለኪያ ቅጽበተ-ፎቶዎች የ `CapacityTelemetrySnapshot` ግቤቶችን በ `metering` ቅጽበታዊ ገጽ እይታ ላይ የተሰኩ ያመርታሉ ፣ እና Prometheus ወደ ውጭ የሚላኩ ምርቶች ለማስመጣት ዝግጁ የሆነውን Grafana ቦርድ በ I18NI000000073X የፕሮጀክት ቡድን ውስጥ nano-SORA ክፍያዎች፣ እና SLA ተገዢነት በእውነተኛ ጊዜ።【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- የመለኪያ ማለስለስ ሲነቃ የምስል ቀረጻው `smoothed_gib_hours` እና `smoothed_por_success_bps`ን ያካትታል ስለዚህ ኦፕሬተሮች EMA-trended values አስተዳደር ለክፍያ ከሚጠቀምባቸው ጥሬ ቆጣሪዎች ጋር ማወዳደር ይችላሉ።【crates/sorafs_node/src/metering.rs:401】

### 5. ሙግት እና መሻር አያያዝ

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| `CapacityDisputeV1` ክፍያን ይግለጹ (ቅሬታ፣ ማስረጃ፣ ዒላማ አቅራቢ)። | አስተዳደር ምክር ቤት | Norito schema + አረጋጋጭ። |
| የCLI ድጋፍ አለመግባባቶችን ለማቅረብ እና ምላሽ ለመስጠት (ከማስረጃ ማያያዣዎች ጋር)። | Tooling WG | ቆራጥ የሆነ የማስረጃ ጥቅል ማሸግ ያረጋግጡ። |
| ለተደጋጋሚ SLA ጥሰቶች (በራስ-ሰር ወደ ሙግት ጨምር) አውቶማቲክ ፍተሻዎችን ያክሉ። | ታዛቢነት | የማንቂያ ገደቦች እና የአስተዳደር መንጠቆዎች። |
| የሰነድ መሻሪያ አጫዋች ደብተር (የእፎይታ ጊዜ ፣የተሰካው መረጃ መልቀቂያ)። | ሰነዶች / የማከማቻ ቡድን | የፖሊሲ ሰነድ እና ኦፕሬተር runbook አገናኝ። |

## የሙከራ እና የ CI መስፈርቶች- ለሁሉም አዲስ የመርሃግብር ማረጋገጫዎች (`sorafs_manifest`) የክፍል ሙከራዎች።
- የሚመስሉ የውህደት ሙከራዎች፡ መግለጫ → የማባዛት ቅደም ተከተል → መለኪያ → ክፍያ።
- የናሙና አቅም መግለጫዎችን/ቴሌሜትሪዎችን ለማደስ እና ፊርማዎች እንደተመሳሰለ (`ci/check_sorafs_fixtures.sh` ማራዘም) ለማረጋገጥ CI የስራ ፍሰት።
- የመመዝገቢያ ኤፒአይ ሙከራዎችን ይጫኑ (10k አቅራቢዎችን ፣ 100k ትዕዛዞችን አስመስለው)።

## ቴሌሜትሪ እና ዳሽቦርዶች

- ዳሽቦርድ ፓነሎች;
  - አቅም በአቅራቢው ጥቅም ላይ እንደዋለ ተገልጧል።
  - የማባዛት ትዕዛዝ የኋላ መዝገብ እና አማካይ የምደባ መዘግየት።
  - SLA ተገዢነት (ጊዜ % ፣ የPoR ስኬት መጠን)።
  - ክፍያ ማጠራቀም እና ቅጣቶች በየዘመናቱ።
- ማንቂያዎች
  - ከዝቅተኛ አቅም በታች አቅራቢ።
  - የማባዛት ትዕዛዝ ተጣብቋል> SLA.
  - የቧንቧ መስመር ብልሽቶች መለኪያ.

## የሰነድ አቅርቦቶች

- አቅምን ለማወጅ፣ ቃል ኪዳኖችን ለማደስ እና አጠቃቀምን ለመቆጣጠር ኦፕሬተር መመሪያ።
- መግለጫዎችን ለማጽደቅ, ትዕዛዞችን ለመስጠት, አለመግባባቶችን ለማስተናገድ የአስተዳደር መመሪያ.
- የኤፒአይ ማጣቀሻ ለአቅም የመጨረሻ ነጥቦች እና የማባዛት ቅደም ተከተል ቅርጸት።
- የገበያ ቦታ ለገንቢዎች የሚጠየቁ ጥያቄዎች።

## GA ዝግጁነት ማረጋገጫ ዝርዝር

የመንገድ ካርታ ንጥል **SF-2c** በሮች ማምረት በሂሳብ አያያዝ ላይ በተጨባጭ ማስረጃ ላይ
የክርክር አያያዝ፣ እና ተሳፍሪ። የቅበላ መስፈርቶቹን ለመጠበቅ ከዚህ በታች ያሉትን ቅርሶች ይጠቀሙ
ከትግበራው ጋር በማመሳሰል.

### የምሽት የሂሳብ አያያዝ እና XOR ማስታረቅ
- የአቅም ሁኔታ ቅጽበተ ፎቶን እና የXOR ደብተርን ለተመሳሳይ መስኮት ወደ ውጭ ይላኩ እና ከዚያ ያሂዱ
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ረዳቱ በጎደሉ/ከሚከፈልባቸው ሰፈራዎች ወይም ቅጣቶች ከዜሮ ውጭ ይወጣል እና Prometheus ያወጣል።
  የጽሑፍ ፋይል ማጠቃለያ.
- ማንቂያ `SoraFSCapacityReconciliationMismatch` (በI18NI0000080X ውስጥ)
  የማስታረቅ መለኪያዎች ክፍተቶችን በሚዘግቡበት ጊዜ ሁሉ እሳቶች; ዳሽቦርዶች ስር ይኖራሉ
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- በ`docs/examples/sorafs_capacity_marketplace_validation/` ስር የJSON ማጠቃለያ እና ሃሽዎችን በማህደር ያስቀምጡ
  ከአስተዳደር ፓኬጆች ጋር.

### ሙግት እና ማስረጃን እየቀጨጨ
- ክርክሮችን በI18NI0000083X በኩል ያስገቡ (ሙከራዎች፡-
  `cargo test -p sorafs_car --test capacity_cli`) ስለዚህ ሸክሞች ቀኖናዊ ሆነው ይቆያሉ።
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` እና ቅጣቱን ያሂዱ
  ስብስቦች (`record_capacity_telemetry_penalises_persistent_under_delivery`) አለመግባባቶችን ለማረጋገጥ እና
  ቆራጮች በቁርጠኝነት እንደገና ይጫወታሉ።
- ለመረጃ ቀረጻ እና መጨመር `docs/source/sorafs/dispute_revocation_runbook.md` ተከተል;
  የአገናኝ ምልክት ማጽደቂያ ወደ የማረጋገጫ ሪፖርቱ ይመለሳል።

### አቅራቢ የመሳፈሪያ እና የጭስ ሙከራዎች
- የማስታወቂያ/የቴሌሜትሪ ቅርሶችን ከ`sorafs_manifest_stub capacity ...` ጋር ያድሱ እና እንደገና ያጫውቱ።
  የ CLI ሙከራዎች ከማቅረቡ በፊት (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)።
- በTorii (`/v2/sorafs/capacity/declare`) ያስገቡ እና ከዚያ `/v2/sorafs/capacity/state` ፕላስ ይያዙ
  Grafana ቅጽበታዊ ገጽ እይታዎች። በ I18NI0000092X ውስጥ የመውጫ ፍሰቱን ይከተሉ።
- በውስጡ የተፈረሙ ቅርሶችን እና የእርቅ ውጤቶችን በማህደር ያስቀምጡ
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## ጥገኛ እና ቅደም ተከተል

1. SF-2bን ጨርስ (የመግቢያ ፖሊሲ) - የገበያ ቦታ የሚወሰነው በተረጋገጡ አቅራቢዎች ላይ ነው።
2. ከTorii ውህደት በፊት ሼማ + የመመዝገቢያ ንብርብር (ይህን ሰነድ) ተግብር።
3. ክፍያዎችን ከማንቃትዎ በፊት የመለኪያ ቧንቧን ያጠናቅቁ።
4. የመጨረሻ ደረጃ፡ የመለኪያ መረጃ በደረጃው ላይ ከተረጋገጠ በኋላ በአስተዳደር ቁጥጥር የሚደረግ የክፍያ ስርጭትን ያንቁ።

የሂደቱ ሂደት ከዚህ ሰነድ ጋር በማጣቀስ በፍኖተ ካርታው ውስጥ መከታተል አለበት። እያንዳንዱ ዋና ክፍል (መርሃግብር፣ የቁጥጥር አውሮፕላን፣ ውህደት፣ መለኪያ፣ የክርክር አያያዝ) የባህሪው ሙሉ ደረጃ ላይ እንደደረሰ የመንገድ ካርታውን ያዘምኑ።