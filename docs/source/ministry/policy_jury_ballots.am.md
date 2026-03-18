---
lang: am
direction: ltr
source: docs/source/ministry/policy_jury_ballots.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff3faabda5f1c277f545b7edbbc93f3b58dee65cec943cfd464a026b2984a146
source_last_modified: "2025-12-29T18:16:35.979378+00:00"
translation_last_reviewed: 2026-02-07
title: Policy Jury Sortition & Ballots
translator: machine-google-reviewed
---

የመንገድ ካርታ ንጥል **MINFO-5 — የፖሊሲ ዳኞች ድምጽ መስጫ መሳሪያ ** ተንቀሳቃሽ ቅርጸት ያስፈልገዋል
ለወሳኝ ዳኞች ምርጫ እና የታሸገ ቁርጠኝነት → ምርጫዎችን አሳይ።  የ
`iroha_data_model::ministry::jury` ሞጁል አሁን ሶስት Norito ጭነትን ይልካል።
ጠቅላላውን የድምፅ አሰጣጥ ሂደት ይሸፍኑ:

1. **`PolicyJurySortitionV1`** - የስዕል ሜታዳታን ይመዘግባል (የፕሮፖዛል መታወቂያ፣
   ክብ መታወቂያ፣የሰውነት ማረጋገጫ ቅጽበተ-ፎቶ መፍጨት፣ የዘፈቀደ ምልክት)
   የኮሚቴ መጠን፣ የተመረጡ ዳኞች፣ እና ለራስ-ሰር ጥቅም ላይ የሚውለው የጥበቃ ዝርዝር
   ውድቀት ።  እያንዳንዱ ዋና ማስገቢያ `PolicyJuryFailoverPlan` ሊያካትት ይችላል።
   ከጸጋው በኋላ ወደ ተጠባባቂው ዝርዝር ደረጃ በመጠቆም
   የወር አበባ ማለፊያዎች.  አወቃቀሩ ሆን ተብሎ የሚወሰን ነው ስለዚህ ኦዲተሮች
   ስዕሉን እንደገና ማጫወት እና አንጸባራቂውን ከተመሳሳይ POP ማደስ ይችላል።
   ቅጽበተ-ፎቶ + ቢኮን.
2. **`PolicyJuryBallotCommitV1`** - የታሸገ ቁርጠኝነት ከድምጽ መስጫዎች በፊት የተፃፈ
   ተገለጡ።  ክብ/ፕሮፖዛል/ዳኞች መለያዎችን፣ የ
   Blake2b-256 የዳኝነት መታወቂያ + የድምጽ ምርጫ + ያልሆነ tuple፣ ቀረጻ
   የጊዜ ማህተም፣ እና የድምጽ መስጫ ሁነታ (`plaintext` ወይም `zk-envelope`
   `zk-ballot` ባህሪ ንቁ ነው)።  `PolicyJuryBallotCommitV1::verify_reveal`
   የተከማቸ መፍጨት ከግኝት ክፍያ ጭነት ጋር እንደሚዛመድ ያረጋግጣል።
3. **`PolicyJuryBallotRevealV1`** - ህዝባዊ ገላጭ ነገርን የያዘ
   የድምጽ ምርጫ፣ በቁርጠኝነት ጊዜ ጥቅም ላይ ያልዋለ እና አማራጭ የZK ማረጋገጫ URIs።
   መገለጦች ቢያንስ 16-ባይት ኖኖ ያስፈልጋቸዋል ስለዚህ አስተዳደር ማከም ይችላል።
   ዳኞች ደህንነታቸው በሌላቸው ቻናሎች ላይ በሚሰሩበት ጊዜም ቢሆን ቁርጠኝነት እንደ አስገዳጅነት።

የ`PolicyJurySortitionV1::validate` ረዳት የኮሚቴ መጠንን ያስፈጽማል፣
የተባዛ ማወቅ (ምንም ዳኛ በሁለቱም በኮሚቴው እና በ
የተጠባባቂዎች ዝርዝር) ፣ የታዘዙ የተጠባባቂ ዝርዝር ደረጃዎች እና ትክክለኛ ውድቀት ማጣቀሻዎች።  የድምጽ መስጫው
የማረጋገጫ ልማዶች ሀሳብ ሲቀርብ ወይም ክብ መታወቂያዎች `PolicyJuryBallotError` ያሳድጋሉ።
መንሸራተት፣ ዳኞች በተሳሳተ መንገድ ለመግለጥ ሲሞክሩ ወይም ሀ
`zk-envelope` ቁርጠኝነት በእሱ ውስጥ ተዛማጅ ማረጋገጫዎችን ማቅረብ አልቻለም
መግለጥ።

### ከደንበኞች ጋር መቀላቀል

- የአስተዳደር መሳሪያዎች የመደርደር መግለጫውን መቀጠል እና ማካተት አለባቸው
  የፖሊሲ ፓኬቶች ታዛቢዎች የPOP ቅጽበተ-ፎቶ መፍጨትን እና እንደገና ማስላት ይችላሉ።
  የዘፈቀደ ምልክት እና የእጩ ስብስብ ወደ ተመሳሳይ እንደሚመራ ያረጋግጡ
  ዳኞች ምደባዎች.
- የዳኞች ደንበኞች `PolicyJuryBallotCommitV1` ወዲያውኑ ይመዘግባሉ
  ለድምፃቸው ያልሆነ ማመንጨት.  የተገኘው ቁርጠኝነት ባይት ሊሆን ይችላል።
  ለTorii እንደ ቤዝ64 እሴት ገብቷል ወይም በቀጥታ ወደ Norito የተካተተ
  ክስተቶች.
- በመገለጥ ደረጃ፣ ዳኞች `PolicyJuryBallotRevealV1` ይለቃሉ።  ኦፕሬተሮች
  ክፍያውን በፊት ወደ `PolicyJuryBallotCommitV1::verify_reveal` ይመግቡ
  ድምጹን በመቀበል፣ መገለጡ እንዳልተለወጠ ወይም እንዳልተነካ ማረጋገጥ።
- የ `zk-ballot` ባህሪ ሲነቃ ዳኞች ቆራጥነትን ማያያዝ ይችላሉ
  ማስረጃ URIs (ለምሳሌ `sorafs://proofs/pj-2026-02/juror-5`) እንዲሁ ወደ ታች
  ኦዲተሮች በ የተጠቀሰውን የዜሮ እውቀት ምስክር ቅርቅብ ሰርስረው ማውጣት ይችላሉ።
  ቁርጠኝነት.ሦስቱም መዋቅሮች `Encode`፣ `Decode` እና `IntoSchema` ያመጣሉ ማለት ነው።
ለአይኤስአይ ፍሰቶች፣ የCLI Tooling፣ ኤስዲኬዎች እና የአስተዳደር REST API ይገኛሉ።
ለቀኖናዊው ዝገት `crates/iroha_data_model/src/ministry/jury.rs` ይመልከቱ
ትርጓሜዎች እና ረዳት ዘዴዎች.

### CLI ለመደርደር መግለጫዎች ድጋፍ

የፍኖተ ካርታ ንጥል **MINFO-5** አስተዳደር እንዲቻል እንደገና ሊባዛ የሚችል መሣሪያን ጠይቋል
እያንዳንዱ የሪፈረንደም ፓኬት ከመታተሙ በፊት ሊረጋገጥ የሚችል የፖሊሲ-ዳኞች ዝርዝር መግለጫዎችን ይላኩ።
የስራ ቦታው አሁን የ`cargo xtask ministry-jury sortition` ትዕዛዝን ያጋልጣል፡

```bash
cargo xtask ministry-jury sortition \
  --roster docs/examples/ministry/policy_jury_roster_example.json \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --beacon 22b1e48d47123f5c9e3f0cc0c8e34aa3c5f9c49a2cbb70559d3cb0ddc1a6ef01 \
  --committee-size 3 \
  --waitlist-size 2 \
  --drawn-at 2026-01-15T09:00:00Z \
  --waitlist-ttl-hours 72 \
  --out artifacts/ministry/policy_jury_sortition.json
```

- `--roster` የሚወስን የፖፕ ዝርዝር ይቀበላል (JSON ምሳሌ፡
  `docs/examples/ministry/policy_jury_roster_example.json`).  እያንዳንዱ ግቤት
  `juror_id`፣ `pop_identity`፣ ክብደት እና አማራጭን ያውጃል
  `grace_period_secs`.  ብቁ ያልሆኑ ግቤቶች በራስ-ሰር ይጣራሉ።
- `--beacon` በአስተዳደር ውስጥ የተያዘውን ባለ 32-ባይት የዘፈቀደ ምልክት ያስገባል
  ደቂቃዎች ።  CLI መብራቱን በቀጥታ ወደ ChaCha20 RNG ስለዚህ ኦዲተሮች ያገናኛል።
  ባይት-ለ-ባይት መሳል እንደገና መጫወት ይችላል።
- `--committee-size`፣ `--waitlist-size` እና `--waitlist-ttl-hours` ይቆጣጠራሉ።
  የተቀመጡ ዳኞች ብዛት፣ ያልተሳካለት ቋት እና ጊዜው የሚያበቃበት ጊዜ ማህተም ተተግብሯል።
  ወደ ተጠባባቂው ዝርዝር ግቤቶች.  ለ ማስገቢያ ያልተሳካ ደረጃ ሲኖር, ትዕዛዙ
  በተዛማጅ የተጠባባቂ ዝርዝር ደረጃ ላይ `PolicyJuryFailoverPlan` መዝግቧል።
- `--drawn-at` ለመደርደር የግድግዳ ሰዓት ጊዜ ማህተም ይመዘግባል; መሳሪያው
  ለማንፀባረቅ ወደ ዩኒክስ ሚሊሰከንዶች ይለውጠዋል።

የመነጨው አንጸባራቂ ሙሉ በሙሉ የተረጋገጠ `PolicyJurySortitionV1` ክፍያ ነው።
ትላልቅ ማሰማራቶች በተለምዶ በ `artifacts/ministry/` ስር ያለውን ውጤት ያስቀምጣሉ
ከግምገማ ፓነል ጋር በቀጥታ ወደ ሪፈረንደም ፓኬቶች ሊጠቃለል ይችላል።
ማጠቃለያ  ገላጭ ውፅዓት በ ውስጥ ይገኛል።
የኤስዲኬ ቡድኖች እንዲችሉ `docs/examples/ministry/policy_jury_sortition_example.json`
በአገር ውስጥ አንድ ሙሉ ስዕል ሳትጫወቱ Norito ዲኮደሮችን ተለማመዱ።

### የድምጽ መስጫ ፈጻሚዎችን/ረዳቶችን ያሳያል

ዳኛ ደንበኞች ለቁርጠኝነት → ገላጭ ፍሰት እንዲሁ ቆራጥ መሣሪያ ያስፈልጋቸዋል።
ተመሳሳይ የ `cargo xtask ministry-jury` ትዕዛዝ አሁን የሚከተሉትን ረዳቶች ያጋልጣል፡

```bash
cargo xtask ministry-jury ballot commit \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --juror citizen:ada \
  --choice approve \
  --nonce-hex aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899 \
  --committed-at 2026-02-01T13:00:00Z \
  --out artifacts/ministry/policy_jury_commit_ada.json \
  --reveal-out artifacts/ministry/policy_jury_reveal_ada.json

cargo xtask ministry-jury ballot verify \
  --commit artifacts/ministry/policy_jury_commit_ada.json \
  --reveal artifacts/ministry/policy_jury_reveal_ada.json
```

- `ballot commit` የ `PolicyJuryBallotCommitV1` JSON ጭነት ያመነጫል።  መቼ
  `--out` ተትቷል ትዕዛዙ ለ stdout ያለውን ቁርጠኝነት ያትማል።  ከሆነ
  `--reveal-out` መሳሪያው ተዛማጁን ይጽፋል
  `PolicyJuryBallotRevealV1`፣ የቀረበውን አንድ ጊዜ እንደገና በመጠቀም እና በመተግበር ላይ
  አማራጭ `--revealed-at` የጊዜ ማህተም (የ `--committed-at` ወይም የ
  የአሁኑ ጊዜ)።
- `--nonce-hex` ማንኛውንም እኩል-ርዝመት የአስራስድስትዮሽ ሕብረቁምፊ ≥16ባይት ይቀበላል።  ሲቀር
  አጋዥ `OsRng` በመጠቀም ባለ 32 ባይት ኖንስ ያመነጫል፣ ይህም ስክሪፕት ለመፃፍ ቀላል ያደርገዋል።
  juror workflows ያለ ብጁ የዘፈቀደ የቧንቧ መስመር።
- `--choice` ለጉዳይ የማይረዳ እና `approve`፣ `reject` ወይም `abstain`ን ይቀበላል።

`ballot verify` ቁርጠኝነትን/በመግለጥ ጥንድን ያረጋግጣል
`PolicyJuryBallotCommitV1::verify_reveal`፣ የዙሩ መታወቂያ ዋስትና፣
የፕሮፖዛል መታወቂያ፣ የዳኝነት መታወቂያ፣ ኖንስ እና የድምጽ ምርጫ ሁሉም ከመገለጡ በፊት ይጣጣማሉ
ወደ Torii ገብቷል።  ሲረጋገጥ ረዳቱ ዜሮ ባልሆነ ሁኔታ ይወጣል
አልተሳካም፣ ወደ CI ወይም የአካባቢ ዳኛ መግቢያዎች ሽቦ ለማድረግ ደህንነቱ የተጠበቀ ያደርገዋል።