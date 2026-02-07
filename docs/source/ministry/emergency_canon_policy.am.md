---
lang: am
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# የአደጋ ጊዜ ቀኖና እና ቲቲኤል ፖሊሲ (MINFO-6a)

የመንገድ ካርታ ማጣቀሻ፡ **MINFO-6a — የአደጋ ጊዜ ቀኖና እና የቲቲኤል ፖሊሲ**።

ይህ ሰነድ አሁን በTorii እና በCLI የሚላኩትን የክህደት ዝርዝር ደንቦችን፣ የቲቲኤል ማስፈጸሚያ እና የአስተዳደር ግዴታዎችን ይገልጻል። ኦፕሬተሮች አዲስ ግቤቶችን ከማተም ወይም የአደጋ ጊዜ ቀኖናዎችን ከመጥራት በፊት እነዚህን ደንቦች መከተል አለባቸው።

## የደረጃ ፍቺዎች

| ደረጃ | ነባሪ TTL | የግምገማ መስኮት | መስፈርቶች |
|------------------|-----------|------------|
| መደበኛ | 180 ቀናት (`torii.sorafs_gateway.denylist.standard_ttl`) | n/a | `issued_at` መቅረብ አለበት። `expires_at` ሊቀር ይችላል; Torii ወደ `issued_at + standard_ttl` ነባሪ እና ረጅም መስኮቶችን ውድቅ ያደርጋል። |
| ድንገተኛ አደጋ | 30 ቀናት (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 ቀናት (`torii.sorafs_gateway.denylist.emergency_review_window`) | ቀድሞ የጸደቀውን ቀኖና (ለምሳሌ `csam-hotline`) የሚያመለክት ባዶ ያልሆነ `emergency_canon` መለያ ያስፈልገዋል። `issued_at` + `expires_at` በ30-ቀን መስኮት ውስጥ ማረፍ አለበት እና የክለሳ ማስረጃዎች በራስ የመነጨውን የመጨረሻ ቀን (`issued_at + review_window`) መጥቀስ አለባቸው። |
| ቋሚ | ማብቂያ የለውም | n/a | ለላቀ አስተዳደር ውሳኔዎች የተያዘ። ግቤቶች ባዶ ያልሆነ `governance_reference` (የድምጽ መታወቂያ፣ ማኒፌስቶ ሃሽ፣ ወዘተ) መጥቀስ አለባቸው። `expires_at` ውድቅ ተደርጓል። |

ነባሪው በ`torii.sorafs_gateway.denylist.*` እና `iroha_cli` በኩል የሚዋቀሩ ይቆያሉ Torii ፋይል እንደገና ከመጫኑ በፊት ልክ ያልሆኑ ግቤቶችን ለመያዝ ወሰኖቹን ያንፀባርቃሉ።

#የስራ ሂደት

1. **ሜታዳታ አዘጋጁ፡** `policy_tier`፣ `issued_at`፣ `expires_at` (የሚመለከተው ከሆነ)፣ እና `emergency_canon`/`governance_reference` በእያንዳንዱ የJNI307000036X ውስጥ በእያንዳንዱ የJNI307000036X ያካትቱ።
2. ** በአገር ውስጥ ያረጋግጡ:** `iroha app sorafs gateway lint-denylist --path <denylist.json>` ን ያሂዱ ስለዚህም CLI ደረጃ-ተኮር ቲቲኤልዎችን እና ፋይሉ ከመሰጠቱ ወይም ከመሰካቱ በፊት አስፈላጊ የሆኑትን መስኮች ያስፈጽማል።
3. **ማስረጃ ያትሙ፡** ኦዲተሮች ውሳኔውን እንዲከታተሉ በመግቢያው ላይ የተጠቀሰውን የቀኖና መታወቂያ ወይም የአስተዳደር ማጣቀሻ ከጋራ ጉዳይ ጥቅል (የአጀንዳ ፓኬት፣ የሪፈረንደም ደቂቃ፣ ወዘተ) ጋር አያይዘው።
4. **የአደጋ ጊዜ ግቤቶችን ይገምግሙ፡** የአደጋ ጊዜ ቀኖናዎች በ30 ቀናት ውስጥ በራስ-ሰር የአገልግሎት ጊዜው ያበቃል። ኦፕሬተሮች የድህረ-ነገሩን ግምገማ በ7-ቀን መስኮት ውስጥ ማጠናቀቅ እና ውጤቱን በሚኒስቴር መከታተያ/SoraFS የማስረጃ ማከማቻ መመዝገብ አለባቸው።
5. ** Torii ን እንደገና ጫን:** አንዴ ከተረጋገጠ በ`torii.sorafs_gateway.denylist.path` በኩል የውድድር ዝርዝሩን ያሰማሩ እና Torii እንደገና ያስጀምሩ/ይጫኑ; የሩጫ ጊዜው ግቤቶችን ከመግባቱ በፊት ተመሳሳይ ገደቦችን ያስፈጽማል።

## መሳሪያ እና ማጣቀሻዎች

- የአሂድ ፖሊሲ ማስፈጸሚያ በ`sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) ውስጥ ይኖራል እና ጫኚው አሁን የ`torii.sorafs_gateway.denylist.*` ግብዓቶችን ሲተነተን የደረጃ ሜታዳታን ይጠቀማል።
- የ CLI ማረጋገጫ በ `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) ውስጥ ያለውን የአሂድ ጊዜ ፍቺ ያንጸባርቃል። TTL ከተዋቀረው መስኮት ሲያልፍ ወይም የግዴታ ቀኖና/የአስተዳደር ማጣቀሻዎች ሲቀሩ ሊንተሩ አይሳካም።
- Config knobs በ `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) ስር ይገለፃሉ ስለዚህ አስተዳደር የተለያዩ ወሰኖችን ካፀደቀ ኦፕሬተሮች TTLs/የግምገማ የጊዜ ገደቦችን ማስተካከል ይችላሉ።
- የህዝብ ናሙና ውድቅ መዝገብ (`docs/source/sorafs_gateway_denylist_sample.json`) አሁን ሁሉንም ሶስት እርከኖች ያሳያል እና ለአዳዲስ ግቤቶች እንደ ቀኖናዊ አብነት ጥቅም ላይ መዋል አለበት።እነዚህ የጥበቃ መንገዶች የመንገድ ካርታ ንጥል **MINFO-6a** የአደጋ ጊዜ ቀኖና ዝርዝርን ኮድ በማድረግ፣ ያልተገደቡ ቲቲኤሎችን በመከላከል እና ለቋሚ ብሎኮች ግልጽ የአስተዳደር ማስረጃዎችን በማስገደድ ያረካሉ።

## የመመዝገቢያ አውቶሜሽን እና ማስረጃ ወደ ውጭ መላክ

የአደጋ ጊዜ ቀኖና ማፅደቆች የሚወስን የመመዝገቢያ ቅጽበታዊ ገጽ እይታ እና ሀ
Torii ውድቅ ዝርዝሩን ከመጫኑ በፊት diff bundle። ስር ያለው መሳሪያ
`xtask/src/sorafs.rs` እና የ CI መታጠቂያ `ci/check_sorafs_gateway_denylist.sh`
ሙሉውን የስራ ሂደት ይሸፍኑ.

### ቀኖናዊ ጥቅል ትውልድ

1. ደረጃ ጥሬ ግቤቶች (በተለምዶ በአስተዳደር የተገመገመው ፋይል) በስራ ላይ
   ማውጫ.
2. JSON ን ቀኖና ያትሙት በ፡-
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   ትዕዛዙ የተፈረመ ተስማሚ `.json` ጥቅል፣ Norito `.to` ያወጣል።
   ኤንቨሎፕ፣ እና በአስተዳደር ገምጋሚዎች የሚጠበቀው የ Merkle-root ጽሑፍ ፋይል።
   ማውጫውን በ `artifacts/ministry/denylist_registry/` (ወይም የእርስዎ
   የተመረጠ የማስረጃ ባልዲ) ስለዚህ `scripts/ministry/transparency_release.py` ይችላል።
   በኋላ በ `--artifact denylist_bundle=<path>` ይውሰዱት።
3. የመነጨውን `checksums.sha256` ከመግፋትዎ በፊት ከጥቅሉ ጎን ያቆዩት።
   ወደ SoraFS/ጋር. CI's `ci/check_sorafs_gateway_denylist.sh` ልምምድ ያደርጋል
   የ `pack` ረዳት ከናሙና ውድቅ ዝርዝሩ ጋር በመቃወም የመሳሪያውን አሠራር ለማረጋገጥ
   እያንዳንዱ ልቀት.

### Diff + የኦዲት ጥቅል

1. አዲሱን ጥቅል ከቀዳሚው የምርት ቅጽበታዊ ገጽ እይታ ጋር ያወዳድሩ
   xtask ልዩነት አጋዥ፡
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   የJSON ዘገባ ሁሉንም ተጨማሪ/ማስወገድ ይዘረዝራል እና ማስረጃውን ያንጸባርቃል
   በ `MinistryDenylistChangeV1` የሚበላ መዋቅር (የተጠቀሰው በ
   `docs/source/sorafs_gateway_self_cert.md` እና የታዛዥነት እቅድ)።
2. ለእያንዳንዱ የቀኖና ጥያቄ `denylist_diff.json` ያያይዙ (ምን ያህል እንደሆነ ያረጋግጣል)
   ግቤቶች ተዳሰዋል፣ የትኛው ደረጃ ተቀይሯል እና ምን ማስረጃ ሃሽ ካርታዎች ለ
   ቀኖናዊ ጥቅል)።
3. ልዩነቶች በራስ ሰር ሲፈጠሩ (CI ወይም የሚለቀቁት የቧንቧ መስመሮች)፣ ወደ ውጪ መላክ
   `denylist_diff.json` መንገድ በ `--artifact denylist_diff=<path>` በኩል
   ግልጽነት አንጸባራቂ ከንጽህና መለኪያዎች ጋር ይመዘግባል። ተመሳሳይ ሲ.አይ
   አጋዥ የCLI ማጠቃለያ ደረጃን የሚያሄድ `--evidence-out <path>` ይቀበላል
   በኋላ ላይ እንዲታተም የተገኘውን JSON ወደተጠየቀው ቦታ ይገለብጣል።

### ሕትመት እና ግልጽነት1. ጥቅሉን + ልዩ ልዩ ቅርሶችን ወደ ሩብ ወሩ ግልጽነት ማውጫ ውስጥ ጣሉት።
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`)። ግልጽነት
   የመልቀቂያ ረዳት የሚከተሉትን ሊያካትት ይችላል-
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. የተፈጠረውን ጥቅል/ልዩነት በየሩብ ዓመቱ ሪፖርት ያመልክቱ
   (`docs/source/ministry/reports/<YYYY-Q>.md`) እና ተመሳሳይ መንገዶችን ከ
   ኦዲተሮች የማስረጃ መንገዱን ያለ መዳረሻ ማጫወት እንዲችሉ የ GAR ድምጽ ፓኬት
   ውስጣዊ CI. `ci/የሶራፍስ_ጌትዌይ_ዲኒሊስት.sh --ማስረጃ-ውጭ
   artifacts/ministry/denylist_registry//denylist_evidence.json` አሁን
   ጥቅል/ልዩነት/ማስረጃውን በደረቅ አሂድ ያከናውናል (`iroha_cli app sorafs gateway በመደወል)
   ማስረጃ` በኮፈኑ ስር) ስለዚህ አውቶማቲክ ማጠቃለያውን ከጎን አድርጎ መቀጠል ይችላል።
   ቀኖናዊ ጥቅሎች.
3. ከታተመ በኋላ የአስተዳደር ክፍያን በ
   `cargo xtask ministry-transparency anchor` (በራስ ሰር በ
   `transparency_release.py` `--governance-dir` ሲቀርብ) ስለዚህ
   denylist መዝገብ መፍጨት ግልጽነት ጋር በተመሳሳይ DAG ዛፍ ውስጥ ይታያል
   መልቀቅ.

ይህን ሂደት ተከትሎ "የመዝገብ አውቶሜትድ እና ማስረጃ ወደ ውጭ መላክ" ይዘጋል።
ክፍተት በ `roadmap.md:450` ተጠርቷል እና እያንዳንዱን የአደጋ ጊዜ ቀኖና ያረጋግጣል
ውሳኔው ሊባዙ በሚችሉ ቅርሶች፣ JSON ልዩነቶች እና ግልጽነት ምዝግብ ማስታወሻ ይደርሳል
ግቤቶች.

### ቲቲኤል እና ቀኖና ማስረጃ አጋዥ

የጥቅል/ልዩነት ጥንድ ከተመረተ በኋላ ለመያዝ የCLI ማስረጃ አጋዥን ያሂዱ
የ TTL ማጠቃለያዎች እና የአደጋ ጊዜ ግምገማ የግምገማ ጊዜዎች አስተዳደር የሚፈልገው፡-

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

ትዕዛዙ JSON ምንጩን ያሰራጫል፣ እያንዳንዱን ግቤት ያረጋግጣል፣ እና የታመቀ ያመነጫል።
ማጠቃለያ የያዘ

- ጠቅላላ ግቤቶች በ`kind` እና በእያንዳንዱ የፖሊሲ ደረጃ ከመጀመሪያዎቹ/ከቅርቡ ጋር
  የጊዜ ማህተም ታይቷል.
- እያንዳንዱን የአደጋ ጊዜ ቀኖና የሚዘረዝር የ`emergency_reviews[]` ዝርዝር
  ገላጭ፣ ውጤታማ ጊዜው የሚያበቃበት፣ የሚፈቀደው ከፍተኛ TTL እና የተሰላው።
  `review_due_by` የመጨረሻ ቀን።

ኦዲተሮች እንዲችሉ `denylist_evidence.json` ከታሸገው ጥቅል/ልዩነት ጋር ያያይዙ
CLI ን እንደገና ሳታደርጉ የቲቲኤልን ተገዢነት ያረጋግጡ። ቀድሞውኑ የሚያመነጩ የ CI ስራዎች
ቅርቅቦች ረዳቱን ሊጠሩ እና ማስረጃውን አርቲፊኬት ማተም ይችላሉ (ለምሳሌ በ
`ci/check_sorafs_gateway_denylist.sh --evidence-out <path>` በመደወል) ማረጋገጥ
እያንዳንዱ ቀኖና የሚጠይቀው ወጥ የሆነ ማጠቃለያ አለው።

### የመርክሌ መዝገብ ማስረጃ

በMINFO-6 የተዋወቀው የመርክል መዝገብ ኦፕሬተሮችን እንዲያትሙ ይፈልጋል
የስር እና የመግቢያ ማረጋገጫዎች ከቲቲኤል ማጠቃለያ ጋር። ከሩጫ በኋላ ወዲያውኑ
የማስረጃው ረዳት፣ የመርክሌ ቅርሶችን ይያዙ፡-

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```ቅጽበተ-ፎቶው JSON የBLAKE3 Merkle ስር፣ የቅጠል ብዛት እና ሁሉንም ይመዘግባል
ገላጭ/ሃሽ ጥንድ ስለዚህ የGAR ድምጾች የተጠለፈውን ትክክለኛ ዛፍ ሊያመለክቱ ይችላሉ።
`--norito-out` ማቅረብ የ`.to` ቅርስ ከJSON ጋር ያከማቻል።
ጌትዌይስ የመመዝገቢያ ምዝግቦቹን በቀጥታ በNorito በኩል ያስገባሉ
stdout `merkle proof` ለማንኛውም አቅጣጫ ቢት እና እህት እህት ሃሽ ያስወጣል።
ዜሮ ላይ የተመሰረተ የመግቢያ መረጃ ጠቋሚ፣ ለእያንዳንዳቸው የማካተት ማረጋገጫ ማያያዝ ቀላል ያደርገዋል
የአደጋ ጊዜ ቀኖና በGAR ማስታወሻ ውስጥ ተጠቅሷል—የአማራጭ Norito ቅጂ ማስረጃውን ያስቀምጣል።
በደብዳቤ ላይ ለማሰራጨት ዝግጁ። ሁለቱንም JSON እና Norito ቅርሶችን በቀጣይ ያከማቹ
ለቲቲኤል ማጠቃለያ እና ልዩነት ጥቅል ስለዚህ ግልጽነት ልቀቶች እና አስተዳደር
መልህቆች አንድ አይነት ሥርን ይጠቅሳሉ.