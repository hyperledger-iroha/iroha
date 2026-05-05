<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# ኩራ / WSV ደህንነት እና የአፈጻጸም ኦዲት (2026-02-19)

## ወሰን

ይህ ኦዲት የሚከተሉትን ያጠቃልላል

- የኩራ ጽናት እና የበጀት መንገዶች: `crates/iroha_core/src/kura.rs`
- የምርት WSV/የግዛት ቁርጠኝነት/መጠይቅ መንገዶች፡`crates/iroha_core/src/state.rs`
- IVM WSV የማሾፍ አስተናጋጅ ወለሎች (የሙከራ/የዴቭ ወሰን)፡ `crates/ivm/src/mock_wsv.rs`

ከወሰን ውጭ፡ ያልተገናኙ ሳጥኖች እና የሙሉ ስርዓት ቤንችማርክ ድጋሚ መካሄድ።

## የአደጋ ማጠቃለያ

- ወሳኝ: 0
- ከፍተኛ: 4
- መካከለኛ: 6
ዝቅተኛ: 2

## ግኝቶች (በከባድነት የታዘዙ)

##ከፍተኛ

1. **ኩራ ጸሐፊ በ I/O ውድቀቶች ላይ ደነገጠ (የመስቀለኛ መንገድ የመገኘት አደጋ)**
- አካል: ኩራ
- አይነት: ደህንነት (DoS), አስተማማኝነት
- ዝርዝር፡- የጸሐፊ loop ድንጋጤ በ append/index/fsync ስህተቶች ላይ መመለስ የሚችሉ ስህተቶችን ከመመለስ ይልቅ፣ ስለዚህ ጊዜያዊ የዲስክ ጥፋቶች የመስቀለኛ መንገድ ሂደቱን ሊያቋርጡ ይችላሉ።
- ማስረጃ፡-
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- ተፅዕኖ፡ የርቀት ጭነት + የአካባቢ የዲስክ ግፊት ብልሽት/ዳግም ማስጀመር ዑደቶችን ሊያስከትል ይችላል።2. **ኩራ ማስወጣት ሙሉ ዳታ/ኢንዴክስ በ `block_store` mutex ስር ይጽፋል**
- አካል: ኩራ
- አይነት: አፈጻጸም, ተገኝነት
- ዝርዝር: `evict_block_bodies` `blocks.data` እና `blocks.index` መቆለፊያን በመያዝ በሙቀት ፋይሎች በኩል እንደገና ይጽፋል።
- ማስረጃ፡-
  - የመቆለፊያ ግዢ: `crates/iroha_core/src/kura.rs:834`
  - ሙሉ እንደገና መፃፍ ቀለበቶች፡ `crates/iroha_core/src/kura.rs:921`፣ `crates/iroha_core/src/kura.rs:942`
  - አቶሚክ መተካት/ማመሳሰል፡ `crates/iroha_core/src/kura.rs:956`፣ `crates/iroha_core/src/kura.rs:960`
- ተጽእኖ፡- የመፈናቀሉ ክስተቶች በትልልቅ ታሪኮች ላይ ለረጅም ጊዜ መፃፍ/ማንበብ ሊያቆሙ ይችላሉ።

3. **የስቴት ቁርጠኝነት በከባድ የቁርጠኝነት ስራ ላይ ግዙፍ `view_lock` ይይዛል**
- አካል: ምርት WSV
- አይነት: አፈጻጸም, ተገኝነት
- ዝርዝር፡ብሎክ መፈጸም ግብይቶችን ሲፈጽም፣ሀሽ ብሎክ እና የዓለም ሁኔታን ሲፈጽም ልዩ የሆነ `view_lock` ይይዛል።
- ማስረጃ፡-
  - ቆልፍ መያዝ ይጀምራል: `crates/iroha_core/src/state.rs:17456`
  - በመቆለፊያ ውስጥ ይስሩ: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- ተፅዕኖ፡ ቀጣይነት ያለው ከባድ ተግባር ጥያቄን/የመግባባትን ምላሽ ሊያሳጣው ይችላል።4. **IVM JSON አስተዳዳሪ ተለዋጭ ስሞች ያለ ደዋይ ቼኮች (የሙከራ/የዴቭ አስተናጋጅ) ልዩ ሚውቴሽን ይፈቅዳል።
- አካል: IVM WSV አስመሳይ አስተናጋጅ
ዓይነት: ደህንነት (በሙከራ/በዴቭ አካባቢዎች ውስጥ የልዩነት ማሳደግ)
- ዝርዝር፡ JSON ተለዋጭ ስም ተቆጣጣሪዎች በቀጥታ ወደ ሚና/ፈቃድ/የእኩያ ሚውቴሽን ዘዴዎች በጠዋዩ የተቀመጡ የፈቃድ ቶከኖች ወደማያስፈልጋቸው መንገድ ይጓዛሉ።
- ማስረጃ፡-
  - የአስተዳዳሪ ተለዋጭ ስሞች፡- `crates/ivm/src/mock_wsv.rs:4274`፣ `crates/ivm/src/mock_wsv.rs:4371`፣ `crates/ivm/src/mock_wsv.rs:4448`
  - ያልተገለሉ ሙታተሮች፡ `crates/ivm/src/mock_wsv.rs:1035`፣ `crates/ivm/src/mock_wsv.rs:1055`፣ `crates/ivm/src/mock_wsv.rs:855`
  - የወሰን ማስታወሻ በፋይል ሰነዶች (የሙከራ/የዴቪ ሐሳብ)፡ `crates/ivm/src/mock_wsv.rs:295`
- ተፅዕኖ፡ የሙከራ ኮንትራቶች/መሳሪያዎች ራሳቸውን ከፍ ሊያደርጉ እና የደህንነት ግምቶችን በውህደት ልጓሞች ላይ ዋጋ ማጥፋት ይችላሉ።

### መካከለኛ

5. ** የኩራ የበጀት ቼኮች በእያንዳንዱ ሠንጠረዡ (ኦ(n) በያንዳንዱ ጻፍ) ላይ ያሉትን ብሎኮች በድጋሚ ኮድ ያደርጉላቸዋል።
- አካል: ኩራ
ዓይነት: አፈጻጸም
- ዝርዝር፡ እያንዳንዱ ወረፋ የሚጠባበቁትን ብሎኮች በመድገም እና እያንዳንዱን በቀኖናዊ ሽቦ መጠን ዱካ በማድረግ በመጠባበቅ ላይ ያሉ ባይትዎችን ያሰላል።
- ማስረጃ፡-
  - የወረፋ ቅኝት: `crates/iroha_core/src/kura.rs:2509`
  - በየብሎክ ኢንኮድ ዱካ፡- `crates/iroha_core/src/kura.rs:2194`፣ `crates/iroha_core/src/kura.rs:2525`
  - የበጀት ቼክ በወረራ ላይ ተጠርቷል፡ `crates/iroha_core/src/kura.rs:2580`፣ `crates/iroha_core/src/kura.rs:2050`
- ተፅዕኖ፡ የውጤት መበላሸትን ከኋላ መዝገብ ስር ይፃፉ።6. ** የኩራ የበጀት ፍተሻዎች በየወረፋው ተደጋጋሚ የብሎክ ማከማቻ ሜታዳታ ይነበባል**
- አካል: ኩራ
ዓይነት: አፈጻጸም
- ዝርዝር፡ እያንዳንዱ ቼክ `block_store` ሲቆለፍ የሚበረክት የኢንዴክስ ብዛት እና የፋይል ርዝመት ያነባል።
- ማስረጃ፡-
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- ተጽዕኖ፡ ሊወገድ የሚችል I/O/በሞቃታማ የወረፋ መንገድ ላይ ከአናት በላይ።

7. **ኩራ ማፈናቀል የተቀሰቀሰው ከወረፋ የበጀት መንገድ መስመር ውስጥ ነው**
- አካል: ኩራ
- አይነት: አፈጻጸም, ተገኝነት
- ዝርዝር: አዲስ ብሎኮችን ከመቀበልዎ በፊት የወረፋ ዱካ በተመሳሳይ ጊዜ ማስወጣትን ሊጠራ ይችላል።
- ማስረጃ፡-
  - የመደወያ ሰንሰለት: `crates/iroha_core/src/kura.rs:2050`
  - በመስመር ውስጥ የማስወጣት ጥሪ: `crates/iroha_core/src/kura.rs:2603`
- ተጽዕኖ፡ በግብይት ላይ የጅራት መዘግየት ሹል ጅራት/በበጀት ሲቃረብ እንዳይገባ ያግዱ።

8. **`State::view` በክርክር ስር ያለ ግምታዊ መቆለፊያ ሳያገኝ ሊመለስ ይችላል**
- አካል: ምርት WSV
- ዓይነት: ወጥነት / የአፈጻጸም ንግድ
- ዝርዝር፡- በጽሑፍ-መቆለፊያ ክርክር ላይ፣ `try_read` ውድቀት በንድፍ ያለ ጥብቅ ጥበቃ እይታን ይመልሳል።
- ማስረጃ፡-
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
ተጽዕኖ፡ የተሻሻለ ኑሮ፣ ነገር ግን ጠሪዎች በክርክር ውስጥ ያሉ ደካማ-አካላት-አካላተ-ጎሳዎችን መታገስ አለባቸው።9. **`apply_without_execution` በ DA ጠቋሚ እድገት ላይ ጠንካራ `expect` ይጠቀማል**
- አካል: ምርት WSV
- አይነት: ደህንነት (DoS በፍርሃት-በማይለዋወጥ-እረፍት) ፣ አስተማማኝነት
- ዝርዝር፡ የDA ጠቋሚ እድገት ልዩነቶች ካልተሳኩ ቁርጠኛ ብሎክ የመንገድ ድንጋጤን ይተግብሩ።
- ማስረጃ፡-
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
ተጽዕኖ፡ ድብቅ ማረጋገጫ/መረጃ ጠቋሚ ሳንካዎች አንጓ ገዳይ ውድቀቶች ሊሆኑ ይችላሉ።

10. **IVM TLV አሳት syscall ከመመደብ በፊት ግልጽ የሆነ የፖስታ መጠን የለውም (የሙከራ/የዴቭ አስተናጋጅ)**
- አካል: IVM WSV አስመሳይ አስተናጋጅ
- አይነት: ደህንነት (ማህደረ ትውስታ DoS), አፈጻጸም
- ዝርዝር፡ የራስጌ ርዝመትን ያነባል ከዚያም ሙሉ የ TLV ክፍያን ያለ አስተናጋጅ ደረጃ በዚህ መንገድ ይመድባል/ይቀዳል።
- ማስረጃ፡-
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- ተፅዕኖ፡ ተንኮል አዘል የፈተና ጭነቶች ትልቅ ምደባዎችን ሊያስገድድ ይችላል።

### ዝቅተኛ

11. ** የኩራ ማሳወቂያ ቻናል ወሰን የለውም (`std::sync::mpsc::channel`)**
- አካል: ኩራ
- ዓይነት: የአፈፃፀም / የማስታወስ ንፅህና
- ዝርዝር፡ የማሳወቂያ ሰርጥ ቀጣይነት ባለው የአምራች ግፊት ወቅት ተደጋጋሚ የማንቂያ ክስተቶችን ሊያከማች ይችላል።
- ማስረጃ፡-
  - `crates/iroha_core/src/kura.rs:552`
- ተፅዕኖ፡ የማስታወስ ችሎታ እድገት አደጋ በእያንዳንዱ ክስተት መጠን ዝቅተኛ ነው ነገር ግን ሊወገድ የሚችል ነው።12. **የቧንቧ ጎን መኪና ወረፋ ጸሃፊው እስኪያልቅ ድረስ በማህደረ ትውስታ ገደብ የለሽ ነው**
- አካል: ኩራ
- ዓይነት: የአፈፃፀም / የማስታወስ ንፅህና
- ዝርዝር: የጎን መኪና ወረፋ `push_back` ምንም ግልጽ ኮፍያ / backpressure የለውም.
- ማስረጃ፡-
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- ተፅዕኖ፡ ለረጅም ጊዜ የጸሐፊ መዘግየት ጊዜ የማስታወስ ችሎታ እድገት።

## ነባር የሙከራ ሽፋን እና ክፍተቶች

### ኩራ

- ነባር ሽፋን;
  - የማከማቻ-በጀት ባህሪ፡- `store_block_rejects_when_budget_exceeded`፣ `store_block_rejects_when_pending_blocks_exceed_budget`፣ `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`፣ `crates/iroha_core/src/kura.rs:6949`፣ `crates/iroha_core/src/kura.rs:6984`)
  - የማስወጣት ትክክለኛነት እና የውሃ ማደስ፡- `evict_block_bodies_does_not_truncate_unpersisted`፣ `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`፣ `crates/iroha_core/src/kura.rs:8126`)
- ክፍተቶች:
  - ያለምንም ድንጋጤ ለአባሪ/ኢንዴክስ/fsync አለመሳካት አያያዝ የስህተት መርፌ ሽፋን የለም።
  - ለትልቅ በመጠባበቅ ላይ ላሉት ወረፋዎች እና የበጀት ቼክ ወጪን ለመፈተሽ ምንም የአፈፃፀም መመለሻ ሙከራ የለም።
  - በመቆለፊያ ክርክር ውስጥ የረጅም ጊዜ ታሪክ የማስወጣት መዘግየት ሙከራ የለም።

### ፕሮዳክሽን WSV

- ነባር ሽፋን;
  - የክርክር ውድቀት ባህሪ፡ `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - የመቆለፊያ-ትዕዛዝ ደህንነት በደረጃ ጀርባ ዙሪያ፡ `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- ክፍተቶች:
  - በከባድ አለም ቁርጠኝነት ከፍተኛ ተቀባይነት ያለው የቁርጥ ቀን ጊዜን የሚያረጋግጥ የቁጥር ክርክር የለም።
  - የDA ጠቋሚ እድገት ተለዋዋጮች ባልተጠበቀ ሁኔታ ከተሰበሩ ከፍርሃት ነጻ የሆነ አያያዝ ምንም አይነት የተሃድሶ ሙከራ የለም

### IVM WSV ሞክ አስተናጋጅ- ነባር ሽፋን;
  ፈቃድ JSON ተንታኝ ትርጓሜ እና አቻ መተንተን (`crates/ivm/src/mock_wsv.rs:5234`፣ `crates/ivm/src/mock_wsv.rs:5332`)
  - በቲኤልቪ ዲኮድ እና በJSON ዲኮድ (`crates/ivm/src/mock_wsv.rs:5962`፣ `crates/ivm/src/mock_wsv.rs:6078`) ዙሪያ የሲስካል ጭስ ሙከራዎች
- ክፍተቶች:
  - ምንም ያልተፈቀደ-አስተዳዳሪ-ተለዋጭ ስም ውድቅ ሙከራዎች የሉም
  - በ `INPUT_PUBLISH_TLV` ውስጥ ከመጠን በላይ የ TLV ኤንቨሎፕ ውድቅ ሙከራዎች የሉም
  - በፍተሻ ነጥብ ዙሪያ ምንም የቤንችማርክ/የጠባቂ ሙከራዎች የሉም/የክሎን ወጪን ወደነበረበት መመለስ

## ቅድሚያ የሚሰጠው የማሻሻያ እቅድ

### ደረጃ 1 (ከፍተኛ ተጽዕኖ ማጠንከሪያ)

1. የኩራ ጸሃፊ `panic!` ቅርንጫፎችን በማገገም በሚቻል የስህተት ስርጭት + የተበላሸ-የጤና ምልክትን ይተኩ።
- የዒላማ ፋይሎች: `crates/iroha_core/src/kura.rs`
- ተቀባይነት;
  - የተወጋ append/index/fsync አለመሳካቶች አይሸበሩም።
  - በቴሌሜትሪ/በምዝግብ ማስታወሻዎች በኩል ስህተቶች ይታያሉ እና ጸሃፊው መቆጣጠር እንደሚቻል ይቆያል

2. ለIVM mock-host TLV ህትመት እና ለ JSON ፖስታ መንገዶች የታሰሩ የኤንቨሎፕ ቼኮችን ይጨምሩ።
- የዒላማ ፋይሎች: `crates/ivm/src/mock_wsv.rs`
- ተቀባይነት;
  - ከመጠን በላይ የተጫኑ ሸክሞች ከመመደብ በፊት ውድቅ ይደረጋሉ - ከባድ ሂደት
  - አዳዲስ ሙከራዎች ሁለቱንም TLV እና JSON ከመጠን በላይ ጉዳዮችን ይሸፍናሉ።

3. ግልጽ የደዋይ ፍቃድ ፍተሻዎችን ለJSON አስተዳዳሪ ተለዋጭ ስሞች (ወይም በር ተለዋጭ ስሞችን ከ ጥብቅ የሙከራ-ብቻ ባህሪ ባንዲራዎች እና በግልጽ ሰነድ ጀርባ) ያስፈጽሙ።
- የዒላማ ፋይሎች: `crates/ivm/src/mock_wsv.rs`
- ተቀባይነት;
  - ያልተፈቀደ ደዋይ በተለዋጭ ስም ሚና/ፈቃድ/የአቻ ሁኔታን መቀየር አይችልም።

### ደረጃ 2 (የሙቅ መንገድ አፈፃፀም)4. የኩራ በጀት ሒሳብን ጭማሪ አድርግ።
- በሰንሰለት ላይ ሙሉ በመጠባበቅ ላይ-ወረፋ እንደገና ስሌትን በተያዙ ቆጣሪዎች ይተኩ/ይቀጥላሉ/በማውረድ።
- ተቀባይነት;
  - በመጠባበቅ ላይ-ባይት ለማስላት በ O(1) አቅራቢያ ወጭ ያስይዙ
  - በመጠባበቅ ላይ ያለው ጥልቀት እያደገ ሲሄድ የማገገሚያ ቤንችማርክ የተረጋጋ መዘግየት ያሳያል

5. የማስለቀቂያ መቆለፊያ ጊዜን ይቀንሱ።
- አማራጮች፡- የተከፋፈለ መጨናነቅ፣ የተሰነጠቀ ቅጂ ከመቆለፊያ መልቀቂያ ወሰኖች ጋር፣ ወይም የበስተጀርባ ጥገና ሁነታ ከታሰረ የፊት ለፊት እገዳ።
- ተቀባይነት;
  - ትልቅ ታሪክ የማፈናቀል መዘግየት ይቀንሳል እና የፊት ለፊት ስራዎች ምላሽ ሰጪ እንደሆኑ ይቆያሉ።

6. ግምታዊውን `view_lock` ወሳኝ ክፍል በተቻለ መጠን ያሳጥሩ።
- ልዩ የሆኑ የያዙ መስኮቶችን ለመቀነስ መለያየትን ወይም ደረጃቸውን የጠበቁ ዴልታዎችን ቅጽበታዊ ገጽ እይታን ይገምግሙ።
- ተቀባይነት;
  - የክርክር መለኪያዎች በከባድ እገዳዎች የ 99p ማቆያ ጊዜ መቀነስ ያሳያሉ

### ደረጃ 3 (የመከላከያ መንገዶች)

7. ለኩራ ጸሃፊ እና የጎን መኪና ወረፋ የኋላ ግፊት/ካፕ የታሰረ/የተደባለቀ የማንቂያ ምልክት ማስተዋወቅ።
8. የቴሌሜትሪ ዳሽቦርዶችን ዘርጋ ለ፡-
- `view_lock` መጠበቅ / ማሰራጫዎችን ይያዙ
- የማፈናቀሉ ቆይታ እና የተመለሱ ባይት በአንድ ሩጫ
- የበጀት-ቼክ enqueue መዘግየት

## የተጠቆሙ የሙከራ ተጨማሪዎች1. `kura_writer_io_failures_do_not_panic` (ክፍል፣ የስህተት መርፌ)
2. `kura_budget_check_scales_with_pending_depth` (የአፈጻጸም መመለሻ)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (ውህደት/ፐርፍ)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (የክርክር መመለሻ)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (የመቋቋም ችሎታ)
6. `mock_wsv_admin_alias_requires_permissions` (የደህንነት መመለሻ)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (DoS ጠባቂ)
8. `mock_wsv_checkpoint_restore_cost_regression` (perf benchmark)

ስለ ወሰን እና በራስ መተማመን ማስታወሻዎች

- የ `crates/iroha_core/src/kura.rs` እና `crates/iroha_core/src/state.rs` ግኝቶች የምርት መንገድ ግኝቶች ናቸው።
- የ`crates/ivm/src/mock_wsv.rs` ግኝቶች በግልፅ የተፈተነ/የዴቭ አስተናጋጅ ስፋት ያላቸው በፋይል ደረጃ ሰነዶች ናቸው።
- በዚህ ኦዲት በራሱ ምንም የ ABI እትም ለውጦች አያስፈልጉም።