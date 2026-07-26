---
lang: am
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-08T21:57:18.412403+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የጄዲጂ ማረጋገጫዎች፡ ጠባቂ፣ መዞር እና ማቆየት።

ይህ ማስታወሻ አሁን በ`iroha_core` የሚላከው የv1 JDG ምስክርነት ጥበቃን ያሳያል።

- **ኮሚቴው ይገለጻል:** Norito-የተመሰጠረ `JdgCommitteeManifest` ጥቅሎች በየውሂብ ቦታ መዞርን ይይዛሉ
  የጊዜ ሰሌዳዎች (`committee_id`፣ የታዘዙ አባላት፣ ደፍ፣ `activation_height`፣ `retire_height`)።
  መግለጫዎች በ`JdgCommitteeSchedule::from_path` ተጭነዋል እና በጥብቅ መጨመርን ያስገድዳሉ
  የማግበሪያ ቁመቶች ከአማራጭ ጸጋ መደራረብ (`grace_blocks`) በጡረታ በመውጣት/በማግበር መካከል
  ኮሚቴዎች.
- ** የማረጋገጫ ጠባቂ:** `JdgAttestationGuard` የውሂብ ቦታን ማሰርን፣ ጊዜው ያለፈበት፣ ያረጁ ወሰኖች፣
  የኮሚቴ መታወቂያ/ገደብ ማዛመድ፣ የፈራሚ አባልነት፣ የሚደገፉ የፊርማ እቅዶች እና አማራጭ
  የኤስዲኤን ማረጋገጫ በ `JdgSdnEnforcer`። የመጠን መያዣዎች፣ ከፍተኛ መዘግየት እና የተፈቀዱ የፊርማ እቅዶች ናቸው።
  የግንባታ መለኪያዎች; `validate(attestation, dataspace, current_height)` ንቁውን ይመልሳል
  ኮሚቴ ወይም የተዋቀረ ስህተት.
  - `scheme_id = 1` (`simple_threshold`)፡ በፈራሚ ፊርማዎች፣ አማራጭ ፈራሚ ቢትማፕ።
  - `scheme_id = 2` (`bls_normal_aggregate`)፡ ነጠላ ቅድመ-የተጠቃለለ BLS-መደበኛ ፊርማ
    የማረጋገጫ ሃሽ; ፈራሚ የቢትማፕ አማራጭ፣ በምስክሩ ውስጥ ላሉ ሁሉም ፈራሚዎች ነባሪ ነው። BLS
    ድምር ማረጋገጫ በማንፀባረቁ ውስጥ በእያንዳንዱ ኮሚቴ አባል የሚሰራ ፖፒ ይጠይቃል። ጠፍቷል ወይም
    ልክ ያልሆኑ ፖፒዎች ማረጋገጫውን አይቀበሉም።
  የፈቃድ ዝርዝሩን በ`governance.jdg_signature_schemes` በኩል ያዋቅሩ።
- ** ማቆያ መደብር፡** `JdgAttestationStore` ምስክርነቶችን በአንድ የውሂብ ቦታ ከተዋቀረ ጋር ይከታተላል
  በእያንዳንዱ የውሂብ ቦታ ቆብ፣ ሲገቡ በጣም የቆዩ ግቤቶችን መቁረጥ። `for_dataspace` ይደውሉ ወይም
  `for_dataspace_and_epoch` የኦዲት/የድጋሚ ማጫወት ቅርቅቦችን ለማውጣት።
- ** ሙከራዎች፡** የክፍል ሽፋን አሁን ትክክለኛ የኮሚቴ ምርጫን፣ ያልታወቀ ፈራሚ ውድቅ ማድረግን፣ የቆየ ነው።
  ማረጋገጫ አለመቀበል፣ የማይደገፉ የእቅድ መታወቂያዎች እና ማቆየት መግረዝ። ተመልከት
  `crates/iroha_core/src/jurisdiction.rs`.

ጠባቂው ከተዋቀረው የፈቃድ ዝርዝር ውጭ እቅዶችን ውድቅ ያደርጋል።