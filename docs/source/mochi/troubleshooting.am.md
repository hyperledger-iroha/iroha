---
lang: am
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2025-12-29T18:16:35.985892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI መላ ፍለጋ መመሪያ

የአካባቢ MOCHI ዘለላዎች ለመጀመር እምቢ ሲሉ፣ በ ሀ ውስጥ ሲጋቡ ይህን የሩጫ መጽሐፍ ይጠቀሙ
loopን እንደገና ያስጀምሩ ወይም የማገድ/ክስተት/ሁኔታ ዝመናዎችን መልቀቅ ያቁሙ። ን ያራዝመዋል
የመንገድ ካርታ ንጥል "ሰነድ እና መልቀቅ" የተቆጣጣሪውን ባህሪያት ወደ ውስጥ በማዞር
`mochi-core` ወደ ተጨባጭ የማገገሚያ ደረጃዎች።

## 1. የመጀመሪያ ምላሽ ሰጪ ዝርዝር

1. MOCHI እየተጠቀመበት ያለውን የውሂብ ስር ይያዙ. ነባሪው ይከተላል
   `$TMPDIR/mochi/<profile-slug>`; ብጁ ዱካዎች በዩአይ ርዕስ አሞሌ ውስጥ ይታያሉ እና
   በ `cargo run -p mochi-ui-egui -- --data-root ...` በኩል.
2. `./ci/check_mochi.sh` ን ከስራ ቦታ ስር ያሂዱ. ይህ ዋናውን ያረጋግጣል ፣
   አወቃቀሮችን ማስተካከል ከመጀመርዎ በፊት UI፣ እና የውህደት ሳጥኖች።
3. ቅድመ-ቅምጥ (`single-peer` ወይም `four-peer-bft`) አስተውል። የተፈጠረው ቶፖሎጂ
   በመረጃ ስር ምን ያህል የአቻ አቃፊዎች/ምዝግብ ማስታወሻዎች መጠበቅ እንዳለቦት ይወስናል።

## 2. የምዝግብ ማስታወሻዎች እና የቴሌሜትሪ ማስረጃዎችን መሰብሰብ

`NetworkPaths::ensure` (`mochi/mochi-core/src/config.rs` ይመልከቱ) የተረጋጋ ይፈጥራል
አቀማመጥ፡

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

ለውጦችን ከማድረግዎ በፊት እነዚህን ደረጃዎች ይከተሉ:

የመጨረሻውን ለመያዝ የ **Logs** ትርን ይጠቀሙ ወይም `logs/<alias>.log`ን ይክፈቱ።
  ለእያንዳንዱ እኩያ 200 መስመሮች. ተቆጣጣሪው stdout/stderr/system channelsን ይጭናል።
  በ`PeerLogStream` በኩል፣ ስለዚህ እነዚህ ፋይሎች ከUI ውፅዓት ጋር ይዛመዳሉ።
- ቅጽበታዊ ገጽ እይታን በ ** ጥገና → ቅጽበታዊ ገጽ እይታ ወደ ውጭ ላክ ** (ወይም ይደውሉ
  `Supervisor::export_snapshot`)። ቅጽበተ-ፎቶው ማከማቻን፣ ያዋቅራል እና
  ወደ `snapshots/<timestamp>-<label>/` ይገባል.
- ጉዳዩ የዥረት መግብሮችን የሚያካትት ከሆነ፣ `ManagedBlockStream` ይቅዱ፣
  `ManagedEventStream`፣ እና `ManagedStatusStream` የጤና አመልካቾች
  ዳሽቦርድ UI የመጨረሻውን ዳግም የማገናኘት ሙከራ እና የስህተት ምክንያት ያዝ
  ለአደጋው መዝገብ ቅጽበታዊ ገጽ እይታ።

## 3. የአቻ ጅምር ችግሮችን መፍታት

አብዛኛዎቹ የአቻ ማስጀመሪያ ውድቀቶች በሶስት ባልዲዎች ውስጥ ይወድቃሉ፡

### የጠፉ ሁለትዮሽ ወይም መጥፎ መሻሮች

`SupervisorBuilder` ወደ `irohad`፣ `kagami`፣ እና (ወደፊት) `iroha_cli` ይወጣል።
UI ሪፖርት ከሆነ "ሂደትን መፍጠር አልቻለም" ወይም "ፍቃድ ተከልክሏል", MOCHIን ጠቁም
በሚታወቁ-ጥሩ ሁለትዮሽዎች:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

ለማስቀረት `MOCHI_IROHAD`፣ `MOCHI_KAGAMI` እና `MOCHI_IROHA_CLI` ማቀናበር ትችላለህ
ባንዲራዎቹን ደጋግመው በመተየብ. ማረም ቅርቅብ ሲገነባ፣ ያወዳድሩ
`BundleConfig` በ `mochi/mochi-ui-egui/src/config/` ውስጥ ባሉት መንገዶች ላይ
`target/mochi-bundle`.

### የወደብ ግጭት

`PortAllocator` ውቅሮችን ከመጻፍዎ በፊት የ loopback በይነገጽን ይመረምራል። ካየህ
`failed to allocate Torii port` ወይም `failed to allocate P2P port`፣ ሌላ
ሂደቱ አስቀድሞ በነባሪ ክልል (8080/1337) ላይ በማዳመጥ ላይ ነው። MOCHIን እንደገና ያስጀምሩ
ግልጽ ከሆኑ መሠረቶች ጋር;

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

ገንቢው ተከታታይ ወደቦችን ከነዚያ መሰረቶች ያስወጣል፣ ስለዚህ ክልል ያስይዙ
ለቅድመ-ቅምጥዎ መጠን (`peer_count` አቻዎች → `peer_count` ወደቦች በአንድ መጓጓዣ)።

### የዘፍጥረት እና የማከማቻ ሙስናመግለጫ ከማውጣቱ በፊት Kagami ከወጣ እኩዮች ወዲያውኑ ይወድቃሉ። ይፈትሹ
`genesis/*.json`/`.toml` በውስጥ ዳታ ስር። ጋር እንደገና አሂድ
`--kagami /path/to/kagami` ወይም **Settings** መገናኛውን በቀኝ ሁለትዮሽ ላይ ያመልክቱ።
ለማከማቻ ብልሹነት፣ የጥገና ክፍሉን ** መጥረግ እና እንደገና ማመንጨት *** ይጠቀሙ።
አቃፊዎችን በእጅ ከመሰረዝ ይልቅ አዝራር (ከታች የተሸፈነው); የሚለውን እንደገና ይፈጥራል
ሂደቶችን እንደገና ከመጀመርዎ በፊት የአቻ ማውጫዎች እና ቅጽበታዊ ፎቶዎች።

### መቃኘት በራስ ሰር ዳግም ይጀምራል

`[supervisor.restart]` በ `config/local.toml` (ወይም የ CLI ባንዲራዎች)
`--restart-mode`፣ `--restart-max`፣ `--restart-backoff-ms`) ምን ያህል ጊዜ እንደሚቆጣጠሩት
ተቆጣጣሪ ያልተሳኩ እኩዮችን ይሞክራል። UI ሲፈልጉ `mode = "never"` ያዘጋጁ
የመጀመርያውን ውድቀት ወዲያውኑ ይግለጹ ወይም `max_restarts`/`backoff_ms` ያሳጥሩ
በፍጥነት አለመሳካት ያለባቸውን የ CI ስራዎች እንደገና ለመሞከር መስኮቱን ለማጥበብ።

## 4. እኩዮችን በአስተማማኝ ሁኔታ ዳግም ማስጀመር

1. የተጎዱትን እኩዮች ከዳሽቦርድ ያቁሙ ወይም UI ን ያቋርጡ። ተቆጣጣሪው
   እኩያ በሚሮጥበት ጊዜ ማከማቻን ለማጽዳት ፈቃደኛ አይደለም (`PeerHandle::wipe_storage`
   ይመልሳል `PeerStillRunning`)።
2. ወደ ** ጥገና → መጥረግ እና እንደገና ማመንጨት ** ይሂዱ። MOCHI የሚከተሉትን ያደርጋል
   - `peers/<alias>/storage` ሰርዝ;
   - በ `genesis/` ስር ውቅሮችን/ጄኔሲስን እንደገና ለመገንባት Kagamiን እንደገና ያሂዱ; እና
   - በተጠበቁ CLI/አካባቢ መሻሮች እኩዮችን እንደገና ያስጀምሩ።
3. ይህንን በእጅዎ ማድረግ ካለብዎት፡-
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   ከዚያ በኋላ `NetworkPaths::ensure` ዛፉን እንደገና እንዲፈጥር MOCHIን እንደገና ያስጀምሩ።

ከመጥረግዎ በፊት ሁልጊዜ የ`snapshots/<timestamp>` ማህደርን ያስቀምጡ፣ በአገር ውስጥም ቢሆን
ልማት - እነዚያ ቅርቅቦች የሚያስፈልጉትን ትክክለኛ `irohad` ምዝግብ ማስታወሻዎች እና ውቅሮች ይይዛሉ
ሳንካዎችን እንደገና ለማባዛት.

### 4.1 ከቅጽበተ-ፎቶዎች ወደነበረበት መመለስ

አንድ ሙከራ ማከማቻን ሲያበላሽ ወይም የታወቀ ጥሩ ሁኔታን እንደገና ማጫወት ሲያስፈልግ ጥገናውን ይጠቀሙ
የንግግር ** ቅጽበተ-ፎቶን ወደነበረበት መልስ** ቁልፍ (ወይም `Supervisor::restore_snapshot` ይደውሉ) ከመቅዳት ይልቅ
ማውጫዎች በእጅ. ወደ ቅርቅቡ ወይም ወደ ንፁህ የአቃፊ ስም ፍጹም መንገድ ያቅርቡ
በ `snapshots/`. ተቆጣጣሪው የሚከተሉትን ያደርጋል:

1. ማንኛውንም ሩጫ እኩዮችን ማቆም;
2. ቅጽበተ-ፎቶው `metadata.json` ከአሁኑ `chain_id` እና የአቻ ብዛት ጋር የሚዛመድ መሆኑን ያረጋግጡ;
3. `peers/<alias>/{storage,snapshot,config.toml,latest.log}` ወደ ገባሪ መገለጫ ይቅዱ; እና
4. ቀደም ብለው እየሮጡ ከሆነ እኩዮችን እንደገና ከመጀመርዎ በፊት `genesis/genesis.json` ወደነበረበት ይመልሱ።

ቅጽበተ-ፎቶው ለተለየ ቅድመ-ቅምጥ ወይም ሰንሰለት መለያ ከተፈጠረ የመልሶ ማግኛ ጥሪው ይመለሳል ሀ
`SupervisorError::Config` ቅርሶችን በጸጥታ ከመቀላቀል ይልቅ ተዛማጅ ጥቅል መያዝ ይችላሉ።
የመልሶ ማግኛ ልምምዶችን ለማፋጠን በአንድ ቅድመ ዝግጅት ቢያንስ አንድ አዲስ ቅጽበታዊ ገጽ እይታ ያስቀምጡ።

## 5. የማገጃ/ክስተት/ሁኔታ ዥረቶችን መጠገን- **ዥረት ቆሟል ነገር ግን እኩዮች ጤናማ።** **ክስተቶችን ይመልከቱ*
  ለቀይ ሁኔታ አሞሌዎች. የሚተዳደረውን ዥረት ለማስገደድ "አቁም" ከዛ "ጀምር" ን ጠቅ ያድርጉ
  እንደገና መመዝገብ; ተቆጣጣሪው እያንዳንዱን የመገናኘት ሙከራ ይመዘግባል (በእኩያ ስም እና
  ስህተት) ስለዚህ የኋሊት ደረጃዎችን ማረጋገጥ ይችላሉ።
- **የሁኔታ ተደራቢ ጊዜው አልፎበታል።** `ManagedStatusStream` ምርጫዎች `/status` እያንዳንዱ
  ሁለት ሰከንድ እና ከ`STATUS_POLL_INTERVAL * በኋላ ውሂብ መቆሙን ያሳያል።
  STATUS_STALE_MULTIPLIER` (ነባሪ ስድስት ሰከንድ)። ባጁ ቀይ ከቀጠለ ያረጋግጡ
  `torii_status_url` በአቻ ውቅር ውስጥ እና መግቢያው ወይም ቪፒኤን አለመሆኑን ያረጋግጡ
  loopback ግንኙነቶችን ማገድ.
- ** የክስተት መፍታት አለመሳካቶች።** UI የመግለጫ ደረጃን (ጥሬ ባይት) ያትማል።
  `BlockSummary` ወይም Norito ዲኮድ) እና አፀያፊው የግብይት ሃሽ። ወደ ውጪ ላክ
  በፈተናዎች ውስጥ ዲኮዱን እንደገና ማባዛት እንዲችሉ ክስተቱን በቅንጥብ ሰሌዳው በኩል
  (`mochi-core` አጋዥ ግንበኞችን ያጋልጣል
  `mochi/mochi-core/src/torii.rs`)።

ዥረቶች በተደጋጋሚ ሲበላሹ፣ ችግሩን በትክክለኛው የአቻ ስም እና ያዘምኑት።
የስህተት ሕብረቁምፊ (`ToriiErrorKind`) ስለዚህ የመንገድ ካርታ ቴሌሜትሪ ችካሎች እንደተሳሰሩ ይቆያሉ
ወደ ተጨባጭ ማስረጃ.