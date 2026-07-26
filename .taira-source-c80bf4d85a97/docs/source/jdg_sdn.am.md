---
lang: am
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2025-12-29T18:16:35.972838+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN ማረጋገጫዎች እና ሽክርክር

ይህ ማስታወሻ የምስጢር ዳታ መስቀለኛ መንገድ (SDN) ማረጋገጫዎችን የማስፈጸሚያ ሞዴል ይይዛል
በስልጣን ዳታ ጠባቂ (JDG) ፍሰት ጥቅም ላይ የዋለ።

## የቁርጠኝነት ቅርጸት
- `JdgSdnCommitment` ወሰን (`JdgAttestationScope`) ያስራል፣ የተመሰጠረው
  payload hash፣ እና የኤስዲኤን የህዝብ ቁልፍ። ማኅተሞች የተተየቡ ፊርማዎች ናቸው።
  (`SignatureOf<JdgSdnCommitmentSignable>`) በጎራ ከተሰየመው ክፍያ በላይ
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- መዋቅራዊ ማረጋገጫ (`validate_basic`) ያስፈጽማል፡-
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - ትክክለኛ የማገጃ ክልሎች
  - ባዶ ያልሆኑ ማህተሞች
  - በሚሰራበት ጊዜ ወሰን እኩልነት ከማስረጃው ጋር
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- ማባዛት የሚስተናገደው በምስክርነት አረጋጋጭ ነው (ፈራሚ+የክፍያ ሃሽ
  ልዩነት) የተያዙ/የተባዙ ቁርጠኝነትን ለመከላከል።

## የመመዝገቢያ እና የማሽከርከር ፖሊሲ
- የ SDN ቁልፎች በ `JdgSdnRegistry` ውስጥ ይኖራሉ ፣ በ `(Algorithm, public_key_bytes)` ቁልፍ።
- `JdgSdnKeyRecord` የነቃውን ቁመት ፣ አማራጭ የጡረታ ቁመት ፣
  እና አማራጭ የወላጅ ቁልፍ።
- ማሽከርከር የሚተዳደረው በ `JdgSdnRotationPolicy` ነው (በአሁኑ ጊዜ፡ `dual_publish_blocks`
  መደራረብ መስኮት). የልጅ ቁልፍ መመዝገብ የወላጅ ጡረታን ያሻሽላል
  `child.activation + dual_publish_blocks`፣ ከጠባቂዎች ጋር፡
  - የጎደሉ ወላጆች ውድቅ ተደርገዋል
  - እንቅስቃሴዎች በጥብቅ መጨመር አለባቸው
  - ከጸጋ መስኮቱ በላይ የሆኑ መደራረቦች ውድቅ ይደረጋሉ።
- የመመዝገቢያ ረዳቶች የተጫኑትን መዝገቦች (`record`, `keys`) ለሁኔታ ይገለጣሉ
  እና ኤፒአይ መጋለጥ።

## የማረጋገጫ ፍሰት
- `JdgAttestation::validate_with_sdn_registry` መዋቅራዊውን ይጠቀለላል
  የማረጋገጫ ቼኮች እና የኤስዲኤን ማስፈጸሚያ። `JdgSdnPolicy` ክሮች
  - `require_commitments`፡ ለPII/ሚስጥራዊ ጭነት መገኘትን ያስገድዳል
  - `rotation`: የወላጅ ጡረታን በሚያዘምንበት ጊዜ ጥቅም ላይ የሚውለው የጸጋ መስኮት
- እያንዳንዱ ቁርጠኝነት ለ፡-
  - መዋቅራዊ ትክክለኛነት + የማረጋገጫ-ወሰን ግጥሚያ
  - የተመዘገበ ቁልፍ መገኘት
  - የተረጋገጠውን የማገጃ ክልል የሚሸፍን ንቁ መስኮት (የጡረታ ወሰኖች ቀድሞውኑ
    ባለሁለት ህትመት ጸጋን ያካትቱ)
  - በጎራ መለያ በተሰጠው የቁርጠኝነት አካል ላይ የሚሰራ ማኅተም
- የተረጋጉ ስህተቶች ለኦፕሬተር ማስረጃ መረጃ ጠቋሚውን ይለጥፋሉ፡
  `MissingSdnCommitments`፣ `UnknownSdnKey`፣ `InactiveSdnKey`፣ `InvalidSeal`፣
  ወይም መዋቅራዊ `Commitment`/`ScopeMismatch` ውድቀቶች።

## ኦፕሬተር runbook
- ** አቅርቦት:** የመጀመሪያውን የ SDN ቁልፍ በ `activated_at` በ ወይም ከዚያ በፊት ያስመዝግቡ
  የመጀመሪያው ሚስጥራዊ እገዳ ቁመት. የጣት አሻራውን ወደ JDG ኦፕሬተሮች ያትሙ።
- ** አሽከርክር: *** የተከታታይ ቁልፍን ያመንጩ ፣ በ `rotation_parent` ያስመዝግቡት
  አሁን ባለው ቁልፍ ላይ በመጠቆም እና የወላጅ ጡረታ እኩል መሆኑን ያረጋግጡ
  `child_activation + dual_publish_blocks`. የመጫኛ ግዴታዎችን እንደገና ያሽጉ
  በተደራራቢ መስኮቱ ወቅት ገባሪ ቁልፍ.
- ** ኦዲት፡** የመመዝገቢያ ቅጽበተ-ፎቶዎችን (`record`፣ `keys`) በTorii/ሁኔታ ያጋልጣል።
  ኦዲተሮች ንቁውን ቁልፍ እና የጡረታ መስኮቶችን ማረጋገጥ እንዲችሉ ወለል። ማንቂያ
  የተረጋገጠው ክልል ከንቁ መስኮት ውጭ ከወደቀ።
- ** ማገገሚያ: ** `UnknownSdnKey` → መዝገቡ የማኅተም ቁልፍን እንደሚያካትት ያረጋግጡ;
  `InactiveSdnKey` → የማግበር ከፍታዎችን ማዞር ወይም ማስተካከል; `InvalidSeal` →
  የተጫኑ ጭነቶችን እንደገና ያሽጉ እና ማረጋገጫዎችን ያድሱ።## የሩጫ ጊዜ አጋዥ
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) ፖሊሲን ያጠቃልላል +
  በ`validate_with_sdn_registry` በኩል መመዝገብ እና ማረጋገጫዎችን ያረጋግጣል።
- መዝገቦች ከ Norito-encoded `JdgSdnKeyRecord` ቅርቅቦች ሊጫኑ ይችላሉ (ይመልከቱ)
  `JdgSdnEnforcer::from_reader`/`from_path`) ወይም ከ ጋር ተሰብስቧል
  `from_records`, በምዝገባ ወቅት የማዞሪያ መከላከያዎችን ተግባራዊ ያደርጋል.
- ኦፕሬተሮች የNorito ቅርቅብ ለTorii/ሁኔታ እንደማስረጃ ሊቆዩ ይችላሉ።
  ተመሳሳዩ የደመወዝ ጭነት በመግቢያው ጥቅም ላይ የዋለውን አስከባሪ ሲመገብ እና
  የጋራ ስምምነት ጠባቂዎች. አንድ ዓለም አቀፋዊ አስፈፃሚ በሚጀመርበት ጊዜ በ በኩል ሊጀመር ይችላል።
  `init_enforcer_from_path`፣ እና `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  የቀጥታ ፖሊሲውን + ቁልፍ መዝገቦችን ለሁኔታ/Torii ወለል ማጋለጥ።

## ሙከራዎች
- በ `crates/iroha_data_model/src/jurisdiction.rs` ውስጥ የድጋሚ ሽፋን:
  `sdn_registry_accepts_active_commitment`፣ `sdn_registry_rejects_unknown_key`፣
  `sdn_registry_rejects_inactive_key`፣ `sdn_registry_rejects_bad_signature`፣
  `sdn_registry_sets_parent_retirement_window`፣
  `sdn_registry_rejects_overlap_beyond_policy`፣ ካለው ጋር
  መዋቅራዊ ማረጋገጫ/SDN የማረጋገጫ ሙከራዎች።