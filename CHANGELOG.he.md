---
lang: he
direction: rtl
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-04T13:46:50.705991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# יומן שינויים

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

כל השינויים הבולטים בפרויקט זה יתועדו בקובץ זה.

## [לא פורסם]- זרוק את SCALE shim; `norito::codec` מיושם כעת עם סידור Norito מקורי.
- החלף את השימושים של `parity_scale_codec` ב-`norito::codec` על פני ארגזים.
- התחל בהעברת כלים לעיבוד Norito מקורי.
- הסר את התלות הנותרת של `parity-scale-codec` מסביבת העבודה לטובת סידור Norito מקורי.
- החלף נגזרות של תכונת SCALE שיורית בהטמעות מקוריות של Norito ושנה את שם מודול ה-codec עם גרסאות.
- מיזוג `iroha_config_base_derive` ו-`iroha_futures_derive` לתוך `iroha_derive` עם פקודות מאקרו מותאמות לתכונות.
- *(multisig)* דחה חתימות ישירות מרשויות multisig עם קוד שגיאה/סיבת שגיאה יציבה, אכיפת מכסי TTL של multisig על פני ממסרים מקוננים, ומכסי TTL משטחים ב-CLI לפני ההגשה (שווי SDK בהמתנה).
- העבר פקודות מאקרו פרוצדורליות של FFI לתוך `iroha_ffi` והסר את ארגז `iroha_ffi_derive`.
- *(schema_gen)* הסר תכונת `transparent_api` מיותרת מהתלות ב-`iroha_data_model`.
- *(data_model)* שמור את מנרמל ICU NFC עבור ניתוח `Name` כדי להפחית את תקורה של אתחול חוזר.
- 📚 התחלה מהירה למסמך JS, פותר תצורה, זרימת עבודה של פרסום ומתכון מודע לתצורה עבור הלקוח Torii.
- *(IrohaSwift)* העלה את יעדי הפריסה המינימליים ל-iOS 15 / macOS 12, אמצו במקביליות של Swift על פני ממשקי API של לקוח Torii, וסמנו דגמים ציבוריים כ-`Sendable`.
- *(IrohaSwift)* נוספו `ToriiDaProofSummaryArtifact` ו-`DaProofSummaryArtifactEmitter.emit` כדי שיישומי Swift יוכלו לבנות/לפלוט חבילות הוכחה DA תואמות CLI מבלי להוציא ל-CLI, עם מסמכים ומבחני רגרסיה המכסים הן בזיכרון והן בדיסק זרימות עבודה.【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】】【F:IrohaSwift/Test s/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* תקן סידורי סידורי Kaigi Option על-ידי הסרת דגל השימוש החוזר בארכיון מ-`KaigiParticipantCommitment`, הוסף בדיקות הלוך-חזור מקוריות ושחרר את ה-JS פענוח החלפה, כך שהוראות Kaigi עכשיו Norito הלוך-חזור לפני הגשה.【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* אפשר למתקשרי `ToriiClient` למחוק כותרות ברירת מחדל (על ידי העברת `null`) כך ש-`getMetrics` יעבור בצורה נקייה בין JSON ל-Prometheus טקסט קבל כותרות.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* נוספו עוזרים ניתנים לחזרה עבור NFTs, יתרות נכסים לכל חשבון ובעלי הגדרות נכסים (עם הגדרות TypeScript, מסמכים ובדיקות) כך שעימוד Torii מכסה כעת את האפליקציה שנותרה נקודות קצה.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:8 0】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* נוספו הוראות ניהול/בוני עסקאות בתוספת מתכון ממשל כדי שלקוחות JS יוכלו לפרוס הצעות, פתקי הצבעה, חקיקה והתמדה במועצה סוף.【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* נוספו עוזרי הגשה/סטטוס ISO 20022 pacs.008 ומתכון תואם, המאפשר למתקשרי JS להפעיל את גשר ISO Torii ללא HTTP מותאם אישית אינסטלציה.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】- *(javascript)* נוספו עוזרי בונה pacs.008/pacs.009 בתוספת מתכון מונחה תצורה כך שמתקשרי JS יוכלו לסנתז מטענים של ISO 20022 עם מטא נתונים מאומתים של BIC/IBAN לפני שהם פוגעים ב- גשר.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js :1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* השלים את לולאת ההטמעה/הבאה/הוכחה של DA: `ToriiClient.fetchDaPayloadViaGateway` מפיקה כעת נקודות אחיזה אוטומטיות (באמצעות הכריכה החדשה של `deriveDaChunkerHandle`), סיכומי הוכחה אופציונליים עושים שימוש חוזר ב-`generateDaProofSummary` המקורית, והקריאו SDK-tests/cantypings מראה `iroha da get-blob/prove-availability` ללא התאמה אישית אינסטלציה.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* מטא-נתונים של לוח התוצאות של `sorafsGatewayFetch` מתעדים כעת את מזהה המניפסט/CID של השער בכל פעם שמשתמשים בספקי שער, כך שחפצי אימוץ מתאימים ל-CLI לכידות.【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchestrator_rollout.md:23】
- *(torii/cli)* אכיפת מעברי חצייה ISO: Torii דוחה כעת הגשות `pacs.008` עם BICs לא ידועים של סוכן והתצוגה המקדימה של DvP CLI מאמתת את `--delivery-instrument-id` באמצעות `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* הוסף צריכת מזומן PvP דרך `POST /v1/iso20022/pacs009`, אכיפת בדיקות `Purp=SECU` ו-BIC של נתוני ייחוס לפני בנייה העברות.【F:crates/iroha_torii/src/iso20022_bridge.rs:1070】【F:crates/iroha_torii/src/lib.rs:4759】
- *(כלי עבודה)* נוסף `cargo xtask iso-bridge-lint` (בתוספת `ci/check_iso_reference_data.sh`) כדי לאמת תמונות ISIN/CUSIP, BIC↔LEI ו-MIC לצד המאגר מתקנים.【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* פרסום npm מוקשה על ידי הכרזה על מטא נתונים של מאגר, רשימת היתרים של קבצים מפורשים, `publishConfig` התומך במקור, יומן שינויים/משמר בדיקה `prepublishOnly` וזרימת עבודה של GitHub Actions שמתרגלת את Node 18/20 CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.dk/javascript:-slowscript:1
- *(ivm/cuda)* BN254 field add/sub/mul מופעלת כעת על ליבות ה-CUDA החדשות עם אצווה בצד המארח באמצעות `bn254_launch_kernel`, המאפשרת האצת חומרה עבור גאדג'טים של Poseidon ו-ZK תוך שמירה על דטרמיניזם fallbacks.【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 תכונות

- *(cli)* הוסף `iroha transaction get` ופקודות חשובות אחרות (#5289)
- [**שבירה**] נכסים ניתנים להפרדה ולא ניתנים להפרדה (#5308)
- [**שבירה**] סיים בלוקים שאינם ריקים על ידי התרת בלוקים ריקים אחריהם (#5320)
- חשיפת סוגי טלמטריה בסכימה ובלקוח (#5387)
- *(iroha_torii)* סטאבים עבור נקודות קצה מוגנות לתכונות (#5385)
- הוסף מדדי זמן התחייבות (#5380)

### 🐛 תיקוני באגים

- עדכון NonZeros (#5278)
- שגיאות הקלדה בקבצי תיעוד (#5309)
- *(קריפטו)* חשוף את `Signature::payload` getter (#5302) (#5310)
- *(ליבה)* הוסף בדיקות לנוכחות תפקיד לפני הענקתו (#5300)
- *(ליבה)* חבר מחדש עמית מנותק (#5325)
- תקן בדיקות pytest הקשורות לנכסי חנות ו-NFT (#5341)
- *(CI)* תקן זרימת עבודה של ניתוח סטטי של פיתון עבור שירה v2 (#5374)
- אירוע עסקה שפג תוקפו מופיע לאחר ביצוע (#5396)

### 💼 אחר- כלול `rust-toolchain.toml` (#5376)
- אזהרה על `unused`, לא `deny` (#5377)

### 🚜 Refactor

- מטריה Iroha CLI (#5282)
- *(iroha_test_network)* השתמש בפורמט יפה עבור יומנים (#5331)
- [**שבירה**] פשט את הסידרה של `NumericSpec` ב-`genesis.json` (#5340)
- שפר את רישום הרישום עבור חיבור p2p כושל (#5379)
- החזר את `logger.level`, הוסף `logger.filter`, הרחבת מסלולי התצורה (#5384)

### 📚 תיעוד

- הוסף את `network.public_address` ל-`peer.template.toml` (#5321)

### ⚡ ביצועים

- *(kura)* מנע כתיבת בלוק מיותר לדיסק (#5373)
- הטמעת אחסון מותאם אישית עבור hashes של עסקאות (#5405)

### ⚙️ משימות שונות

- תקן את השימוש בשירה (#5285)
- הסר קונסטטים מיותרים מ-`iroha_torii_const` (#5322)
- הסר `AssetEvent::Metadata*` שאינו בשימוש (#5339)
- גרסת Bump Sonarqube Action (#5337)
- הסר הרשאות שאינן בשימוש (#5346)
- הוסף חבילת unzip ל-ci-image (#5347)
- תקן כמה הערות (#5397)
- העבר בדיקות אינטגרציות מתוך ארגז `iroha` (#5393)
- השבת את עבודת defectdojo (#5406)
- הוסף אישור DCO עבור התחייבויות חסרות
- ארגון מחדש של זרימות עבודה (ניסיון שני) (#5399)
- אל תפעיל Pull Request CI בדחיפה לראשי (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### נוסף

- סיים בלוקים שאינם ריקים על ידי התרת בלוקים ריקים אחריהם (#5320)

## [2.0.0-rc.1.2] - 2025-02-25

### תוקן

- עמיתים שנרשמו מחדש משתקפים כעת כהלכה ברשימת העמיתים (#5327)

## [2.0.0-rc.1.1] - 2025-02-12

### נוסף

- הוסף `iroha transaction get` ופקודות חשובות אחרות (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### נוסף

- יישם תחזיות שאילתות (#5242)
- השתמש בביצוע מתמשך (#5082)
- הוסף זמני האזנה ל-iroha cli (#5241)
- הוסף /peers API ל-torii (#5235)
- כתובת אגנוסטית p2p (#5176)
- שפר את השירות והשימושיות של multisig (#5027)
- הגן על `BasicAuth::password` מפני הדפסה (#5195)
- מיון יורד בשאילתת `FindTransactions` (#5190)
- הכנס כותרת בלוק לכל הקשר ביצוע חוזה חכם (#5151)
- זמן התחייבות דינמי מבוסס על אינדקס שינוי צפייה (#4957)
- הגדר ערכת הרשאות ברירת מחדל (#5075)
- הוסף יישום של Niche עבור `Option<Box<R>>` (#5094)
- פרדיקטים לעסקה וחסימה (#5025)
- דווח על כמות הפריטים הנותרים בשאילתה (#5016)
- זמן דיסקרטי מוגבל (#4928)
- הוסף פעולות מתמטיות חסרות ל-`Numeric` (#4976)
- אימות הודעות סנכרון חסימה (#4965)
- מסנני שאילתות (#4833)

### השתנה

- פשט את ניתוח מזהה עמיתים (#5228)
- העבר שגיאת עסקה מחוץ למטען החסימה (#5118)
- שנה את שם JsonString ל-Json (#5154)
- הוסף ישות לקוח לחוזים חכמים (#5073)
- מוביל כשירות הזמנת עסקאות (#4967)
- לגרום לקורה להוריד בלוקים ישנים מהזיכרון (#5103)
- השתמש ב-`ConstVec` להוראות ב-`Executable` (#5096)
- הודעות רכילות לכל היותר פעם אחת (#5079)
- צמצם את השימוש בזיכרון של `CommittedTransaction` (#5089)
- הפוך שגיאות סמן שאילתה לספציפיות יותר (#5086)
- ארגון מחדש של ארגזים (#4970)
- הצג את שאילתת `FindTriggers`, הסר את `FindTriggerById` (#5040)
- אל תסתמך בחתימות לעדכון (#5039)
- שנה פורמט פרמטרים ב-genesis.json (#5020)
- שלח רק הוכחה לשינוי תצוגה נוכחית וקודמת (#4929)
- השבת שליחת הודעה כאשר אינך מוכן כדי למנוע לולאה תפוסה (#5032)
- העבר את כמות הנכס הכוללת להגדרת הנכס (#5029)
- חתום רק על הכותרת של הבלוק, לא על כל המטען (#5000)
- השתמש ב-`HashOf<BlockHeader>` כסוג ה-hash של הבלוק (#4998)
- פשט את `/health` ואת `/api_version` (#4960)
- שנה את השם של `configs` ל-`defaults`, הסר את `swarm` (#4862)

### תוקן- לשטח את התפקיד הפנימי ב-json (#5198)
- תקן אזהרות `cargo audit` (#5183)
- הוסף בדיקת טווח לאינדקס החתימה (#5157)
- תיקון דוגמה של מאקרו של מודל במסמכים (#5149)
- סגור את ה-ws כראוי בזרם בלוקים/אירועים (#5101)
- בדיקת עמיתים מהימנים שבור (#5121)
- בדוק שלגוש הבא יש גובה +1 (#5111)
- תקן חותמת זמן של בלוק בראשית (#5098)
- תקן הידור `iroha_genesis` ללא תכונת `transparent_api` (#5056)
- טיפול נכון ב-`replace_top_block` (#4870)
- תקן שיבוט של מבצע (#4955)
- הצג פרטי שגיאה נוספים (#4973)
- השתמש ב-`GET` עבור זרם בלוקים (#4990)
- שפר את הטיפול בעסקאות בתור (#4947)
- למנוע הודעות חסימת חסימה מיותרות (#4909)
- למנוע מבוי סתום בשליחת הודעה גדולה בו זמנית (#4948)
- הסר עסקה שפג תוקפן מהמטמון (#4922)
- תקן את כתובת האתר של torii עם הנתיב (#4903)

### הוסר

- הסר API מבוסס מודול מהלקוח (#5184)
- הסר את `riffle_iter` (#5181)
- הסר תלות שאינה בשימוש (#5173)
- הסר את הקידומת `max` מ-`blocks_in_memory` (#5145)
- הסר אומדן קונצנזוס (#5116)
- הסר את `event_recommendations` מהגוש (#4932)

### אבטחה

## [2.0.0-pre-rc.22.1] - 2024-07-30

### תוקן

- הוסיף את `jq` לתמונת הדוקר

## [2.0.0-pre-rc.22.0] - 2024-07-25

### נוסף

- ציין פרמטרים על השרשרת במפורש בראשית (#4812)
- אפשר דג טורבו עם `Instruction` מרובים (#4805)
- יישום מחדש של עסקאות ריבוי חתימות (#4788)
- הטמעת פרמטרים מובנים לעומת מותאמים אישית על השרשרת (#4731)
- שפר את השימוש בהוראות מותאמות אישית (#4778)
- הפוך את המטא נתונים לדינמיים באמצעות הטמעת JsonString (#4732)
- אפשר למספר עמיתים להגיש בלוק בראשית (#4775)
- ספק `SignedBlock` במקום `SignedTransaction` לעמית (#4739)
- הוראות מותאמות אישית ב-executor (#4645)
- הרחב את ה-cli של הלקוח לבקשת שאילתות json (#4684)
 - הוסף תמיכה בזיהוי עבור `norito_decoder` (#4680)
- הכללה של סכימת הרשאות למודל נתוני מבצע (#4658)
- נוספו הרשאות טריגר רישום במבצע ברירת המחדל (#4616)
 - תמיכה ב-JSON ב-`norito_cli`
- הצג פסק זמן סרק של p2p

### השתנה

- החלף את `lol_alloc` ב-`dlmalloc` (#4857)
- שנה את שם `type_` ל-`type` בסכימה (#4855)
- החלף את `Duration` ב-`u64` בסכימה (#4841)
- השתמש ב-EnvFilter דמוי `RUST_LOG` לרישום (#4837)
- שמור על חסימת הצבעה כשאפשר (#4828)
- הגירה מעיוות לאקסום (#4718)
- מודל נתוני מבצע מפוצל (#4791)
- מודל נתונים רדוד (#4734) (#4792)
- אל תשלח מפתח ציבורי עם חתימה (#4518)
- שנה את שם `--outfile` ל-`--out-file` (#4679)
- שנה את שם השרת והלקוח של iroha (#4662)
- שנה את שם `PermissionToken` ל-`Permission` (#4635)
- דחה את `BlockMessages` בשקיקה (#4606)
- להפוך את `SignedBlock` לבלתי ניתן לשינוי (#4620)
- שנה את שם TransactionValue ל-CommittedTransaction (#4610)
- אימות חשבונות אישיים באמצעות מזהה (#4411)
- השתמש בפורמט multihash עבור מפתחות פרטיים (#4541)
 - שנה את שם `parity_scale_decoder` ל-`norito_cli`
- שלח בלוקים לאימות סט B
- להפוך את `Role` לשקוף (#4886)
- הפק את ה-hash של בלוק מהכותרת (#4890)

### תוקן- בדוק שהרשות היא הבעלים של הדומיין להעברה (#4807)
- הסר אתחול הכפול של לוגר (#4800)
- תקן מוסכמות שמות עבור נכסים והרשאות (#4741)
- שדרוג המבצע בטרנזקציה נפרדת בבלוק בראשית (#4757)
- ערך ברירת מחדל נכון עבור `JsonString` (#4692)
- שפר את הודעת השגיאה של הסידריאליזציה (#4659)
- אל תיבהל אם המפתח הציבורי Ed25519Sha512 שעבר הוא באורך לא חוקי (#4650)
- השתמש באינדקס שינוי תצוגה תקין בעת טעינת בלוק התחלתית (#4612)
- אל תפעיל בטרם עת מפעילי זמן לפני חותמת הזמן `start` שלהם (#4333)
- תמיכה ב-`https` עבור `torii_url` (#4601) (#4617)
- הסר את serde(flaten) מ-SetKeyValue/RemoveKeyValue (#4547)
- ערכת ההדק מסודרת כהלכה
- ביטול `PermissionToken`s שהוסרו ב-`Upgrade<Executor>` (#4503)
- דווח על מדד שינוי תצוגה נכון עבור הסיבוב הנוכחי
- הסר טריגרים מתאימים ב-`Unregister<Domain>` (#4461)
- בדוק את מפתח פאב בראשית בסבב בראשית
- למנוע רישום בראשית דומיין או חשבון
- הסר הרשאות מתפקידים בביטול הרישום של הישות
- מטא נתונים של טריגר נגישים בחוזים חכמים
- השתמש בנעילת rw כדי למנוע תצוגת מצב לא עקבית (#4867)
- ידית מזלג רך בתמונת מצב (#4868)
- תקן את MinSize עבור ChaCha20Poly1305
- הוסף מגבלות ל-LiveQueryStore כדי למנוע שימוש גבוה בזיכרון (#4893)

### הוסר

- הסר מפתח ציבורי מהמפתח הפרטי ed25519 (#4856)
- הסר את kura.lock (#4849)
- החזר את הסיומות `_ms` ו-`_bytes` בתצורה (#4667)
- הסר את סיומת `_id` ו-`_file` משדות בראשית (#4724)
- הסר נכסי אינדקס ב-AssetsMap על ידי AssetDefinitionId (#4701)
- הסר את הדומיין מזהות הטריגר (#4640)
- הסר חתימה בראשית מ-Iroha (#4673)
- הסר את `Visit` מאוגד מ-`Validate` (#4642)
- הסר את `TriggeringEventFilterBox` (#4866)
- הסר את `garbage` בלחיצת יד p2p (#4889)
- הסר את `committed_topology` מהגוש (#4880)

### אבטחה

- הישמרו מפני דליפת סודות

## [2.0.0-pre-rc.21] - 2024-04-19

### נוסף

- כלול מזהה טריגר בנקודת הכניסה של טריגר (#4391)
- חשיפת ערכת אירועים כשדות סיביות בסכימה (#4381)
- הצג את `wsv` חדש עם גישה פרטנית (#2664)
- הוסף מסנני אירועים לאירועי `PermissionTokenSchemaUpdate`, `Configuration` ו-`Executor`
- הצג את "מצב" תמונת מצב (#4365)
- אפשר הענקת/ביטול הרשאות לתפקיד (#4244)
- הצג סוג מספרי דיוק שרירותי עבור נכסים (הסר את כל סוגי המספרים האחרים) (#3660)
- מגבלת דלק שונה עבור המבצע (#3354)
- שלב את pprof profiler (#4250)
- הוסף פקודה משנה נכס ב-CLI של הלקוח (#4200)
- הרשאות `Register<AssetDefinition>` (#4049)
- הוסף `chain_id` כדי למנוע התקפות שידור חוזר (#4185)
- הוסף פקודות משנה לעריכת מטא נתונים של דומיין ב-CLI של הלקוח (#4175)
- ליישם סט חנות, להסיר, לקבל פעולות ב-Client CLI (#4163)
- ספור חוזים חכמים זהים עבור טריגרים (#4133)
- הוסף פקודה משנה ל-CLI של הלקוח כדי להעביר דומיינים (#3974)
- תמיכה בפרוסות קופסאות ב-FFI (#4062)
- git commit SHA ל-CLI של הלקוח (#4042)
- מאקרו proc עבור לוח המחדל של אימות ברירת המחדל (#3856)
- הציג את בונה בקשות שאילתות ל-Client API (#3124)
- שאילתות עצלות בתוך חוזים חכמים (#3929)
- פרמטר שאילתה `fetch_size` (#3900)
- הוראה להעברת נכסים בחנות (#4258)
- שמירה מפני דליפת סודות (#3240)
- ביטול כפילויות של טריגרים עם אותו קוד מקור (#4419)

### השתנה- שרשרת כלי חלודה חבטה ללילה-2024-04-18
- שלח בלוקים לאימות סט B (#4387)
- פיצול אירועי צינור לאירועי בלוק ועסקאות (#4366)
- שנה את שם מקטע התצורה של `[telemetry.dev]` ל-`[dev_telemetry]` (#4377)
- הפוך את `Action` ו-`Filter` לא-גנריים (#4375)
- שפר את API לסינון אירועים עם דפוס בונה (#3068)
- לאחד ממשקי API של מסנני אירועים שונים, להציג ממשק API של בונה שוטף
- שנה את שם `FilterBox` ל-`EventFilterBox`
- שנה את שם `TriggeringFilterBox` ל-`TriggeringEventFilterBox`
- שפר את שמות המסננים, למשל. `AccountFilter` -> `AccountEventFilter`
- שכתוב את התצורה בהתאם לתצורת RFC (#4239)
- הסתר מבנה פנימי של מבני הגירסה מהממשק ה-API הציבורי (#3887)
- הכנס באופן זמני הזמנה צפויה לאחר יותר מדי שינויים בתצוגה כושלת (#4263)
- השתמש בסוגי מפתחות בטון ב-`iroha_crypto` (#4181)
- שינויים בתצוגה מפוצלת מהודעות רגילות (#4115)
- הפוך את `SignedTransaction` לבלתי ניתן לשינוי (#4162)
- ייצא `iroha_config` עד `iroha_client` (#4147)
- ייצא `iroha_crypto` עד `iroha_client` (#4149)
- ייצא `data_model` עד `iroha_client` (#4081)
- הסר את התלות `openssl-sys` מ-`iroha_crypto` והכנס קצה אחורי של TLS הניתנים להגדרה ל-`iroha_client` (#3422)
- החלף את EOF `hyperledger/ursa` לא מתוחזק בפתרון פנימי `iroha_crypto` (#3422)
- מטב את ביצועי המבצע (#4013)
- עדכון עמיתים בטופולוגיה (#3995)

### תוקן

- הסר טריגרים מתאימים ב-`Unregister<Domain>` (#4461)
- הסר הרשאות מתפקידים בעת ביטול הרישום של ישות (#4242)
- טען שהטרנסקציית בראשית חתומה על ידי מפתח פאב Genesis (#4253)
- הצג פסק זמן עבור עמיתים שאינם מגיבים ב-p2p (#4267)
- מניעת רישום דומיין או חשבון Genesis (#4226)
- `MinSize` עבור `ChaCha20Poly1305` (#4395)
- הפעל את המסוף כאשר `tokio-console` מופעל (#4377)
- הפרד כל פריט עם `\n` וצור באופן רקורסיבי ספריות אב עבור יומני קבצים `dev-telemetry`
- מנע רישום חשבון ללא חתימות (#4212)
- יצירת צמד מפתחות אינה ניתנת לטעויות (#4283)
- הפסק את קידוד מפתחות `X25519` בתור `Ed25519` (#4174)
- בצע אימות חתימה ב-`no_std` (#4270)
- קריאה לשיטות חסימה בהקשר אסינכרון (#4211)
- בטל אסימונים משויכים בעת ביטול רישום של ישות (#3962)
- באג חסימת אסינכרון בעת הפעלת Sumeragi
- `(get|set)_config` 401 HTTP קבוע (#4177)
- שם ארכיון `musl` ב-Docker (#4193)
- הדפסת ניפוי באגים בחוזה חכם (#4178)
- עדכון טופולוגיה בהפעלה מחדש (#4164)
- רישום של עמית חדש (#4142)
- סדר איטרציה צפוי בשרשרת (#4130)
- ארכיטקט מחדש של לוגר ותצורה דינמית (#4100)
- טריגר אטומיות (#4106)
- בעיית הזמנת הודעות בחנות שאילתות (#4057)
- הגדר `Content-Type: application/x-norito` עבור נקודות קצה אשר משיבות באמצעות Norito

### הוסר

- פרמטר תצורה `logger.tokio_console_address` (#4377)
- `NotificationEvent` (#4377)
- `Value` enum (#4305)
- צבירת MST מ-iroha (#4229)
- שיבוט עבור ISI וביצוע שאילתות בחוזים חכמים (#4182)
- תכונות `bridge` ו-`dex` (#4152)
- אירועים שטוחים (#3068)
- ביטויים (#4089)
- הפניה לתצורה שנוצרה אוטומטית
- `warp` רעש ביומנים (#4097)

### אבטחה

- למנוע זיוף מפתחות פאב ב-p2p (#4065)
- ודא שחתימות `secp256k1` שיוצאות מ-OpenSSL מנורמלות (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### נוסף- העברת בעלות `Domain`
- `Domain` הרשאות בעלים
- הוסף שדה `owned_by` ל-`Domain`
- ניתוח מסנן כ-JSON5 ב-`iroha_client_cli` (#3923)
- הוסף תמיכה בשימוש ב-Self type ב-Serde מתויגים חלקית
- תקן ממשק API של בלוק (#3884)
- יישם את `Fast` מצב init cura
- הוסף כותרת כתב ויתור של iroha_swarm
- תמיכה ראשונית בתמונות WSV

### תוקן

- תקן הורדת מבצע ב-update_configs.sh (#3990)
- rustc תקין ב-devShell
- תקן כוויות צריבה `Trigger`
- תיקון העברה `AssetDefinition`
- תקן את `RemoveKeyValue` עבור `Domain`
- תקן את השימוש ב-`Span::join`
- תקן באג אי התאמה בטופולוגיה (#3903)
- תקן את רף `apply_blocks` ו-`validate_blocks`
- `mkdir -r` עם נתיב חנות, לא נתיב נעילה (#3908)
- אל תיכשל אם dir קיים ב-test_env.py
- תקן מחרוזת אימות/הרשאה (#3876)
- הודעת שגיאה טובה יותר עבור שגיאת חיפוש שאילתה
- הוסף מפתח ציבורי לחשבון Genesis ל-dev docker compose
- השווה מטען אסימון הרשאה כ-JSON (#3855)
- תקן את `irrefutable_let_patterns` במאקרו `#[model]`
- אפשר ל-genesis לבצע כל ISI (#3850)
- תקן אימות בראשית (#3844)
- תקן טופולוגיה עבור 3 עמיתים או פחות
- תקן את אופן חישוב ההיסטוגרמה tx_amounts.
- התקלפות `genesis_transactions_are_validated()`
- יצירת אימות ברירת מחדל
- תקן כיבוי חינני של iroha

### Refactor- הסר תלות שאינה בשימוש (#3992)
- תלות ב-bump (#3981)
- שנה את שם המאמת למבצע (#3976)
- הסר את `IsAssetDefinitionOwner` (#3979)
- כלול קוד חוזה חכם בסביבת העבודה (#3944)
- מיזוג נקודות קצה של API וטלמטריה לשרת אחד
- העבר את Expression len מתוך API ציבורי לליבה (#3949)
- הימנע משיבוט בחיפוש תפקידים
- שאילתות טווח לתפקידים
- העבר תפקידי חשבון ל-`WSV`
- שנה את שם ISI מ-*Box ל-*Expr (#3930)
- הסר את הקידומת 'גרסת' ממכולות עם גרסאות (#3913)
- העבר את `commit_topology` למטען בלוק (#3916)
- העבר מאקרו `telemetry_future` ל-syn 2.0
- רשום עם ניתן לזיהוי בגבולות ISI (#3925)
- הוסף תמיכה גנרית בסיסית ל-`derive(HasOrigin)`
- נקה את התיעוד של Emitter APIs כדי לשמח את clippy
- הוסף בדיקות עבור מאקרו derive(HasOrigin), צמצם את החזרות ב-derive(IdEqOrdHash), תקן דיווח שגיאות על יציב
- שפר את מתן השמות, פשט את מפות הסינון החוזרות ונשנות והיפטר מ-.למעט ב-deriva(Filter) מיותרים
- הפוך את PartiallyTaggedSerialize/Deserialize לשימוש יקירי
- הפוך לנגזר (IdEqOrdHash) להשתמש יקירי, הוסף בדיקות
- הפוך לנגזר (מסנן) להשתמש יקירי
- עדכן את iroha_data_model_derive כדי להשתמש ב-syn 2.0
- הוסף בדיקות תנאי בדיקת חתימה
- אפשר רק קבוצה קבועה של תנאי אימות חתימה
- הכללה ConstBytes ל-ConstVec שמכיל כל רצף const
- השתמש בייצוג יעיל יותר עבור ערכי בתים שאינם משתנים
- אחסן את ה-wsv הסופי בתמונת מצב
- הוסף שחקן `SnapshotMaker`
- מגבלת מסמכים של ניתוח נגזרת בפקודות מאקרו פרוק
- לנקות הערות
- לחלץ כלי בדיקה נפוץ לניתוח תכונות ל-lib.rs
- השתמש ב-parse_display ועדכון Attr -> מתן שמות של Attrs
- אפשר שימוש בהתאמת דפוסים ב-ffi function args
- צמצם את החזרות בניתוח getset attrs
- שנה את שם Emitter::into_token_stream ל-Emitter::finish_token_stream
- השתמש ב-parse_display כדי לנתח אסימוני getset
- תקן שגיאות הקלדה ושפר הודעות שגיאה
- iroha_ffi_derive: השתמש ב-darling כדי לנתח תכונות והשתמש ב-syn 2.0
- iroha_ffi_derive: החלף proc-macro-error ב-manyhow
- פשט את קוד קובץ kura lock
- להפוך את כל הערכים המספריים לסידרה בתור מילוליות של מחרוזת
- פיצול Kagami (#3841)
- שכתוב את `scripts/test-env.sh`
- הבדיל בין חוזה חכם לנקודות כניסה להדק
- Elide `.cloned()` ב-`data_model/src/block.rs`
- עדכן את `iroha_schema_derive` כדי להשתמש ב-syn 2.0

## [2.0.0-pre-rc.19] - 2023-08-14

### נוסף- hyperledger#3309 Bump IVM זמן ריצה משופר
- hyperledger#3383 הטמע מאקרו כדי לנתח כתובות של שקע בזמן הידור
- hyperledger#2398 הוסף מבחני אינטגרציה עבור מסנני שאילתות
- כלול את הודעת השגיאה בפועל ב-`InternalError`
- שימוש ב-`nightly-2023-06-25` בתור שרשרת הכלים המוגדרת כברירת מחדל
- hyperledger#3692 העברת Validator
- [התמחות DSL] hyperledger#3688: יישם חשבון בסיסי כמאקרו פרוק
- hyperledger#3371 אימות מפוצל `entrypoint` כדי להבטיח שמאמתים לא יראו עוד כחוזים חכמים
- hyperledger#3651 תמונות WSV, המאפשרות להעלות צומת Iroha במהירות לאחר קריסה
- hyperledger#3752 החלף את `MockValidator` במאמת `Initial` שמקבל את כל העסקאות
- hyperledger#3276 הוסף הוראה זמנית בשם `Log` שמתעדת מחרוזת שצוינה ביומן הראשי של הצומת Iroha
- hyperledger#3641 הפוך את מטען אסימון ההרשאה לקריאה אנושית
- hyperledger#3324 הוסף בדיקות `iroha_client_cli` קשורות ל-`burn` ושיחזור
- hyperledger#3781 אימות עסקאות בראשית
- hyperledger#2885 הבדל בין אירועים שניתן ולא ניתן להשתמש בהם עבור טריגרים
- hyperledger#2245 בנייה מבוססת `Nix` של צומת iroha בינארי כ-`AppImage`

### תוקן

- hyperledger#3613 רגרסיה שעלולה לאפשר קבלת עסקאות שנחתמו בצורה שגויה
- דחה מוקדם את טופולוגיית התצורה השגויה
- hyperledger#3445 תקן רגרסיה וגרם ל-`POST` בנקודת הקצה `/configuration` לעבוד שוב
- hyperledger#3654 תקן את `iroha2` `glibc` מבוסס `Dockerfiles` לפריסה
- hyperledger#3451 תיקון `docker` בנוי על מחשבי Mac סיליקון של Apple
- hyperledger#3741 תקן שגיאת `tempfile` ב-`kagami validator`
- hyperledger#3758 תקן רגרסיה שבה לא ניתן היה לבנות ארגזים בודדים, אך ניתן לבנות אותם כחלק מסביבת העבודה
- hyperledger#3777 פרצת תיקון ברישום התפקיד לא מאומתת
- hyperledger#3805 תיקון Iroha לא נכבה לאחר קבלת `SIGTERM`

### אחר

- hyperledger#3648 כלול בדיקת `docker-compose.*.yml` בתהליכי ה-CI
- העבר הוראות `len()` מ-`iroha_data_model` ל-`iroha_core`
- hyperledger#3672 החלף את `HashMap` ב-`FxHashMap` בפקודות מאקרו נגזרות
- hyperledger#3374 Unify doc-comments של שגיאה ויישום `fmt::Display`
- hyperledger#3289 השתמש בירושה של חלל העבודה Rust 1.70 לאורך כל הפרויקט
- hyperledger#3654 הוסף `Dockerfiles` לבניית iroha2 על `GNU libc <https://www.gnu.org/software/libc/>`_
- הצג את `syn` 2.0, `manyhow` ו-`darling` עבור proc-macros
- hyperledger#3802 Unicode `kagami crypto` seed

## [2.0.0-pre-rc.18]

### נוסף

- hyperledger#3468: סמן בצד השרת, המאפשר הערכה בעצלתיים של עימוד של נכנסים מחדש, שאמורות להיות לו השלכות ביצועים חיוביות משמעותיות על זמן האחזור של השאילתה
- hyperledger#3624: אסימוני הרשאה לשימוש כללי; ספציפית
  - לאסימוני הרשאות יכולים להיות כל מבנה
  - מבנה האסימון מתואר בעצמו ב-`iroha_schema` ומסודר כמחרוזת JSON
  - ערך האסימון מקודד `Norito`
  - כתוצאה משינוי זה הועברה מוסכמות שמות אסימון הרשאות מ-`snake_case` ל-`UpeerCamelCase`
- hyperledger#3615 שימור wsv לאחר אימות

### תוקן- hyperledger#3627 אטומיות עסקה נאכפת כעת באמצעות שיבוט של `WorlStateView`
- hyperledger#3195 הרחב את התנהגות הפאניקה בעת קבלת עסקת בראשית שנדחתה
- hyperledger#3042 תקן הודעת בקשה שגויה
- hyperledger#3352 פיצול זרימת בקרה והודעת נתונים לערוצים נפרדים
- hyperledger#3543 שפר את הדיוק של מדדים

## 2.0.0-pre-rc.17

### נוסף

- hyperledger#3330 הארכת דה-סידריאליזציה של `NumericValue`
- hyperledger#2622 תמיכה ב-`u128`/`i128` ב-FFI
- hyperledger#3088 הכנס החמצת תור, כדי למנוע DoS
- Hyperledger#2373 גרסאות הפקודות `kagami swarm file` ו-`kagami swarm dir` להפקת קבצי `docker-compose`
- hyperledger#3597 ניתוח אסימון ההרשאה (צד Iroha)
- hyperledger#3353 הסר את `eyre` מ-`block.rs` על ידי ספירת תנאי שגיאה ושימוש בשגיאות עם הקלדה חזקה
- hyperledger#3318 שלב עסקאות שנדחו וקבלו בבלוקים כדי לשמור על סדר עיבוד העסקאות

### תוקן

- hyperledger#3075 פאניקה על עסקה לא חוקית ב-`genesis.json` כדי למנוע עיבוד של עסקאות לא חוקיות
- hyperledger#3461 טיפול נכון בערכי ברירת המחדל בתצורת ברירת המחדל
- hyperledger#3548 תקן תכונה שקופה של `IntoSchema`
- hyperledger#3552 תקן ייצוג סכימת נתיב המאמת
- hyperledger#3546 תיקון לטריגרים של זמן נתקעים
- hyperledger#3162 אסרו 0 גובה בבקשות סטרימינג בלוק
- בדיקה ראשונית של תצורת מאקרו
- hyperledger#3592 תיקון עבור קבצי תצורה המתעדכנים ב-`release`
- hyperledger#3246 אל תערב את `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ ללא `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_
- hyperledger#3570 הצג נכון שגיאות שאילתת מחרוזת בצד הלקוח
- hyperledger#3596 `iroha_client_cli` מציג בלוקים/אירועים
- hyperledger#3473 לגרום ל-`kagami validator` לעבוד מחוץ לספריית השורש של מאגר iroha

### אחר

- hyperledger#3063 מפה עסקה `hash` לגובה החסימה ב-`wsv`
- מוקלד חזק `HashOf<T>` ב-`Value`

## [2.0.0-pre-rc.16]

### נוסף

- פקודה hyperledger#2373 `kagami swarm` להפקת `docker-compose.yml`
- hyperledger#3525 תקן את ה-API של עסקאות
- hyperledger#3376 הוסף Iroha לקוח CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ מסגרת אוטומציה
- hyperledger#3516 שמור את ה-Bob Hash המקורי ב-`LoadedExecutable`

### תוקן

- hyperledger#3462 הוסף פקודת נכס `burn` ל-`client_cli`
- hyperledger#3233 סוגי שגיאות Refactor
- hyperledger#3330 תקן רגרסיה, על ידי יישום ידני של `serde::de::Deserialize` עבור `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums`
- hyperledger#3487 החזר סוגים חסרים לתוך הסכימה
- hyperledger#3444 החזר מבחן לסכימה
- hyperledger#3496 תקן ניתוח שדה `SocketAddr`
- hyperledger#3498 תקן זיהוי soft-fork
- hyperledger#3396 אחסן חסימה ב-`kura` לפני פליטת אירוע מחויב לחסימה

### אחר

- hyperledger#2817 הסר שינוי פנימי מ-`WorldStateView`
- hyperledger#3363 Genesis API refactor
- Refactor קיים והשלמה עם בדיקות חדשות לטופולוגיה
- עבור מ-`Codecov <https://about.codecov.io/>`_ ל-`Coveralls <https://coveralls.io/>`_ לכיסוי בדיקה
- hyperledger#3533 שנה את השם של `Bool` ל-`bool` בסכימה

## [2.0.0-pre-rc.15]

### נוסף- hyperledger#3231 מאמת מונוליטי
- hyperledger#3015 תמיכה באופטימיזציה של נישה ב-FFI
- hyperledger#2547 הוסף לוגו ל-`AssetDefinition`
- hyperledger#3274 הוסף ל-`kagami` פקודה משנה שיוצרת דוגמאות (מועברת בחזרה ל-LTS)
- hyperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ פתית
- hyperledger#3412 העבר את רכילות העסקה לשחקן נפרד
- hyperledger#3435 הצג את המבקר `Expression`
- hyperledger#3168 ספק אימות בראשית כקובץ נפרד
- hyperledger#3454 הפוך את LTS לברירת המחדל עבור רוב הפעולות והתיעוד של Docker
- hyperledger#3090 הפצת פרמטרים בשרשרת מבלוקצ'יין ל-`sumeragi`

### תוקן

- hyperledger#3330 תקן דה-סריאליזציה של enum לא מתויג עם עלי `u128` (מובאים בחזרה ל-RC14)
- hyperledger#2581 הפחתת רעש ביומנים
- hyperledger#3360 תקן את רף `tx/s`
- hyperledger#3393 ניתוק לולאת מבוי סתום לתקשורת ב-`actors`
- hyperledger#3402 תיקון `nightly` build
- hyperledger#3411 לטפל כראוי בחיבור בו-זמני של עמיתים
- hyperledger#3440 הוצא משימוש המרות נכסים במהלך ההעברה, במקום זאת טופלו על ידי חוזים חכמים
- hyperledger#3408: תקן את בדיקת `public_keys_cannot_be_burned_to_nothing`

### אחר

- hyperledger#3362 העבר לשחקנים `tokio`
- hyperledger#3349 הסר את `EvaluateOnHost` מחוזים חכמים
- hyperledger#1786 הוסף `iroha` סוגים מקוריים עבור כתובות socket
- השבת מטמון IVM
- הפעל מחדש את המטמון IVM
- שנה את שם מאמת ההרשאות למאמת
- hyperledger#3388 הפוך את `model!` למאקרו של תכונה ברמת המודול
- hyperledger#3370 סידורי `hash` כמחרוזת הקסדצימלית
- העבר את `maximum_transactions_in_block` מתצורת `queue` ל-`sumeragi`
- הוצא משימוש והסר סוג `AssetDefinitionEntry`
- שנה את שם `configs/client_cli` ל-`configs/client`
- עדכון `MAINTAINERS.md`

## [2.0.0-pre-rc.14]

### נוסף

- דגם נתונים hyperledger#3127 `structs` אטום כברירת מחדל
- hyperledger#3122 השתמש ב-`Algorithm` לאחסון פונקציית עיכול (תורם קהילה)
- hyperledger#3153 פלט `iroha_client_cli` ניתן לקריאה במכונה
- hyperledger#3105 יישם `Transfer` עבור `AssetDefinition`
- hyperledger#3010 `Transaction` התווסף אירוע תפוגת צינור

### תוקן

- גרסה hyperledger#3113 של בדיקות רשת לא יציבות
- hyperledger#3129 תיקון `Parameter` de/serialisation
- hyperledger#3141 יישום ידני של `IntoSchema` עבור `Hash`
- hyperledger#3155 תקן וו פאניקה בבדיקות, מניעת מבוי סתום
- hyperledger#3166 אל תראה שינוי במצב לא פעיל, שיפור הביצועים
- hyperledger#2123 חזרה ל-PublicKey de/serialization מ-multihash
- hyperledger#3132 הוסף NewParameter validator
- hyperledger#3249 פיצול גיבוב של בלוק לגרסאות חלקיות ושלמות
- hyperledger#3031 תקן את UI/UX של פרמטרי תצורה חסרים
- hyperledger#3247 הוסרה הזרקת תקלה מ-`sumeragi`.

### אחר

- הוסף `#[cfg(debug_assertions)]` חסר כדי לתקן כשלים מזויפים
- hyperledger#2133 שכתוב את הטופולוגיה כדי להיות קרובה יותר לספר הלבן
- הסר את התלות של `iroha_client` ב-`iroha_core`
- hyperledger#2943 גזר את `HasOrigin`
- hyperledger#3232 שתף מטא נתונים של סביבת עבודה
- hyperledger#3254 Refactor `commit_block()` ו-`replace_top_block()`
- השתמש במטפל ברירת מחדל יציב של מקצין
- hyperledger#3183 שנה את שם הקבצים `docker-compose.yml`
- שיפר את פורמט התצוגה `Multihash`
- hyperledger#3268 מזהי פריט ייחודיים בעולם
- תבנית יחסי ציבור חדשה

## [2.0.0-pre-rc.13]

### נוסף- hyperledger#2399 פרמטרי תצורה כ-ISI.
- hyperledger#3119 הוסף מדד `dropped_messages`.
- hyperledger#3094 צור רשת עם עמיתים `n`.
- hyperledger#3082 ספק נתונים מלאים באירוע `Created`.
- hyperledger#3021 ייבוא ​​מצביע אטום.
- hyperledger#2794 דחה רשימות חסרות שדה עם אפליה מפורשת ב-FFI.
- hyperledger#2922 הוסף את `Grant<Role>` ליצירת ברירת המחדל.
- hyperledger#2922 השמט את השדה `inner` ב-`NewRole` דה-סריליזציה של json.
- hyperledger#2922 השמט את `object(_id)` ב-Deserialization של json.
- hyperledger#2922 השמט את `Id` ב-deserialization של json.
- hyperledger#2922 השמט את `Identifiable` ב-Deserialization של json.
- hyperledger#2963 הוסף את `queue_size` למדדים.
- hyperledger#3027 ליישם קובץ נעילה עבור Kura.
- hyperledger#2813 Kagami ליצור תצורת עמיתים כברירת מחדל.
- hyperledger#3019 תמיכה ב-JSON5.
- hyperledger#2231 צור FFI Wrapper API.
- hyperledger#2999 צבור חתימות בלוק.
- hyperledger#2995 זיהוי מזלג רך.
- hyperledger#2905 הרחבת פעולות אריתמטיות לתמיכה ב-`NumericValue`
- hyperledger#2868 שלח גרסת iroha וביצוע hash ביומנים.
- hyperledger#2096 שאילתה עבור הסכום הכולל של הנכס.
- hyperledger#2899 הוסף תת-פקודה מרובת הוראות לתוך 'client_cli'
- hyperledger#2247 הסר רעשי תקשורת ב-websocket.
- hyperledger#2889 הוסף תמיכה בזרימה בלוק לתוך `iroha_client`
- hyperledger#2280 הפקת אירועי הרשאה כאשר התפקיד מוענק/מבוטל.
- hyperledger#2797 העשרה אירועים.
- hyperledger#2725 הכנס מחדש פסק זמן ל-`submit_transaction_blocking`
- hyperledger#2712 בדיקות תצורה.
- hyperledger#2491 תמיכה ב-Enum ב-FFi.
- hyperledger#2775 צור מפתחות שונים בראשית סינתטית.
- hyperledger#2627 סיום תצורה, נקודת כניסת פרוקסי, קאגמי docgen.
- hyperledger#2765 צור בראשית סינתטית ב-`kagami`
- hyperledger#2698 תקן הודעת שגיאה לא ברורה ב-`iroha_client`
- hyperledger#2689 הוסף פרמטרים של הגדרת אסימון הרשאה.
- hyperledger#2502 אחסן GIT hash של build.
- hyperledger#2672 הוסף וריאנט ופרדיקטים `ipv4Addr`, `ipv6Addr`.
- hyperledger#2626 יישם פקודות מאקרו `Combine`, מפוצלות `config`.
- hyperledger#2586 `Builder` ו-`LoadFromEnv` עבור מבני proxy.
- hyperledger#2611 נגזר `TryFromReprC` ו-`IntoFfi` עבור מבנים אטומים גנריים.
- hyperledger#2587 פיצול `Configurable` לשתי תכונות. #2587: פיצול `Configurable` לשתי תכונות
- hyperledger#2488 הוסף תמיכה עבור impls תכונה ב-`ffi_export`
- hyperledger#2553 הוסף מיון לשאילתות נכסים.
- hyperledger#2407 מפעילים פרמטרים.
- hyperledger#2536 הצג את `ffi_import` עבור לקוחות FFI.
- hyperledger#2338 הוסף מכשור `cargo-all-features`.
- hyperledger#2564 אפשרויות אלגוריתם כלי Kagami.
- hyperledger#2490 יישם ffi_export עבור פונקציות עצמאיות.
- hyperledger#1891 אימות ביצוע טריגר.
- hyperledger#1988 הפקת פקודות מאקרו עבור Identifiable, Eq, Hash, Ord.
- hyperledger#2434 FFI bindgen library.
- hyperledger#2073 העדיפו ConstString על פני מחרוזת עבור סוגים בבלוקצ'יין.
- hyperledger#1889 הוסף טריגרים בהיקף של דומיין.
- hyperledger#2098 שאילתות כותרת חסום. #2098: הוסף שאילתות כותרות בלוק
- hyperledger#2467 הוסף תת-פקודה להענקת חשבון לתוך iroha_client_cli.
- hyperledger#2301 הוסף את ה-hash החסימה של העסקה בעת שאילתה.
 - hyperledger#2454 הוסף סקריפט בנייה לכלי המפענח Norito.
- hyperledger#2061 הפקת מאקרו עבור מסננים.- hyperledger#2228 הוסף גרסה לא מורשית לשגיאת שאילתת חוזים חכמים.
- hyperledger#2395 הוסף פאניקה אם לא ניתן להחיל בראשית.
- hyperledger#2000 אסור לאפשר שמות ריקים. #2000: אל תאפשר שמות ריקים
 - hyperledger#2127 הוסף בדיקת שפיות כדי להבטיח שכל הנתונים שפוענחו על ידי ה-Codec Norito נצרכים.
- hyperledger#2360 הפוך את `genesis.json` לאופציונלי שוב.
- hyperledger#2053 הוסף בדיקות לכל השאילתות הנותרות בבלוקצ'יין פרטי.
- hyperledger#2381 Unify `Role` רישום.
- hyperledger#2053 הוסף בדיקות לשאילתות הקשורות לנכסים בבלוקצ'יין פרטי.
- hyperledger#2053 הוסף בדיקות ל-'private_blockchain'
- hyperledger#2302 הוסף שאילתת בדל 'FindTriggersByDomainId'.
- hyperledger#1998 הוסף מסננים לשאילתות.
- hyperledger#2276 כלול hash בלוק נוכחי לתוך BlockHeaderValue.
- hyperledger#2161 מזהה טיפול ו-FNS משותפים.
- הוסף מזהה ידית והטמיע מקבילות FFI של תכונות משותפות (Clone, Eq, Ord)
- hyperledger#1638 `configuration` החזרת עץ משנה של מסמך.
- hyperledger#2132 הוסף מאקרו `endpointN` proc.
- hyperledger#2257 Revoke<Role> פולט אירוע RoleRevoked.
- hyperledger#2125 הוסף שאילתת FindAssetDefinitionById.
- hyperledger#1926 הוסף טיפול באותות וכיבוי חינני.
- hyperledger#2161 ליצור פונקציות FFI עבור `data_model`
- hyperledger#1149 ספירת קבצי החסימה אינה עולה על 1000000 לכל ספרייה.
- hyperledger#1413 הוסף נקודת קצה של גרסת API.
- hyperledger#2103 תומך בשאילתות עבור בלוקים ועסקאות. הוסף שאילתה `FindAllTransactions`
- hyperledger#2186 הוסף העברה ISI עבור `BigQuantity` ו-`Fixed`.
- hyperledger#2056 הוסף ארגז מאקרו לנגזר פרוק עבור `AssetValueType` `enum`.
- hyperledger#2100 הוסף שאילתה כדי למצוא את כל החשבונות עם נכס.
- hyperledger#2179 ייעול ביצוע טריגר.
- hyperledger#1883 הסר קובצי תצורה מוטבעים.
- hyperledger#2105 מטפל בשגיאות שאילתה בלקוח.
- hyperledger#2050 הוסף שאילתות הקשורות לתפקידים.
- hyperledger#1572 אסימוני הרשאה מיוחדים.
- hyperledger#2121 בדוק שצמד המפתחות תקף כאשר הוא נבנה.
 - hyperledger#2003 הצג את כלי המפענח Norito.
- hyperledger#1952 הוסף רף TPS כסטנדרט עבור אופטימיזציות.
- hyperledger#2040 הוסף מבחן אינטגרציה עם מגבלת ביצוע עסקאות.
- hyperledger#1890 הצג מבחני אינטגרציה המבוססים על מקרי שימוש של Orillion.
- hyperledger#2048 הוסף קובץ Toolchain.
- hyperledger#2100 הוסף שאילתה כדי למצוא את כל החשבונות עם נכס.
- hyperledger#2179 ייעול ביצוע טריגר.
- hyperledger#1883 הסר קובצי תצורה מוטבעים.
- hyperledger#2004 לאסור על `isize` ו-`usize` להפוך ל-`IntoSchema`.
- hyperledger#2105 מטפל בשגיאות שאילתה בלקוח.
- hyperledger#2050 הוסף שאילתות הקשורות לתפקידים.
- hyperledger#1572 אסימוני הרשאה מיוחדים.
- hyperledger#2121 בדוק שצמד המפתחות תקף כאשר הוא נבנה.
 - hyperledger#2003 הצג את כלי המפענח Norito.
- hyperledger#1952 הוסף רף TPS כסטנדרט עבור אופטימיזציות.
- hyperledger#2040 הוסף מבחן אינטגרציה עם מגבלת ביצוע עסקאות.
- hyperledger#1890 הצג מבחני אינטגרציה המבוססים על מקרי שימוש של Orillion.
- hyperledger#2048 הוסף קובץ Toolchain.
- hyperledger#2037 הצג טריגרים מראש.
- hyperledger#1621 הצג באמצעות הפעלת שיחות.
- hyperledger#1970 הוסף נקודת קצה סכימה אופציונלית.
- hyperledger#1620 הצג טריגרים מבוססי זמן.
- hyperledger#1918 יישם אימות בסיסי עבור `client`
- hyperledger#1726 יישם זרימת עבודה של שחרור יחסי ציבור.
- hyperledger#1815 הפוך את תגובות השאילתה למבנה טיפוסי יותר.- hyperledger#1928 יישם יצירת יומן שינויים באמצעות `gitchangelog`
- hyperledger#1902 סקריפט התקנה של 4 עמיתים מתכת חשופה.

  נוספה גרסה של setup_test_env.sh שאינה דורשת docker-compose ומשתמשת ב-bug build של Iroha.
- hyperledger#1619 הצג טריגרים מבוססי אירועים.
- hyperledger#1195 סגור חיבור websocket בצורה נקייה.
- hyperledger#1606 הוסף קישור ipfs ללוגו הדומיין במבנה הדומיין.
- hyperledger#1754 הוסף את מפקח Kura CLI.
- hyperledger#1790 שפר את הביצועים באמצעות וקטורים מבוססי מחסנית.
- hyperledger#1805 צבעי קצה אופציונליים לשגיאות פאניקה.
- hyperledger#1749 `no_std` ב-`data_model`
- hyperledger#1179 הוסף הוראת ביטול-הרשאה או תפקיד.
- hyperledger#1782 להפוך את iroha_crypto no_std לתואם.
- hyperledger#1172 יישום אירועי הוראות.
- hyperledger#1734 אמת את `Name` כדי לא לכלול רווחים לבנים.
- hyperledger#1144 הוסף קינון של מטא נתונים.
- #1210 חסימת סטרימינג (צד השרת).
- hyperledger#1331 יישם עוד מדדי `Prometheus`.
- hyperledger#1689 תקן תלות בתכונות. #1261: הוסף נפיחות מטען.
- hyperledger#1675 השתמש בסוג במקום במבנה עטיפה עבור פריטים בגרסה.
- hyperledger#1643 המתן עד שעמיתים יבצעו בראשית במבחנים.
- hyperledger#1678 `try_allocate`
- hyperledger#1216 הוסף נקודת קצה Prometheus. #1216: יישום ראשוני של נקודת קצה מדדים.
- hyperledger#1238 עדכונים ברמת יומן ריצה בזמן ריצה. יצרה טעינה מחדש מבוססת נקודות כניסה בסיסית `connection`.
- hyperledger#1652 עיצוב כותרת PR.
- הוסף את מספר העמיתים המחוברים ל-`Status`

  - חזור ל"מחק דברים הקשורים למספר העמיתים המחוברים"

  זה מחזיר את commit b228b41dab3c035ce9973b6aa3b35d443c082544.
  - ל-Clarify `Peer` יש מפתח ציבורי אמיתי רק לאחר לחיצת יד
  - `DisconnectPeer` ללא בדיקות
  - ליישם ביצוע ביטול רישום עמיתים
  - הוסף (בטל) פקודת משנה ל-`client_cli`
  - סרב לחיבורים חוזרים מעמית לא רשום לפי כתובתו

  לאחר שהחבר שלך מבטל את הרישום ומנתק עמית אחר,
  הרשת שלך תשמע בקשות חיבור מחדש מהעמית.
  כל מה שאתה יכול לדעת בהתחלה הוא הכתובת שמספר היציאה שלה הוא שרירותי.
  אז זכור את העמית הלא רשום לפי החלק שאינו מספר היציאה
  ומסרבים להתחבר מחדש משם
- הוסף נקודת קצה `/status` ליציאה ספציפית.

### תיקונים- hyperledger#3129 תקן את `Parameter` de/serialization.
- hyperledger#3109 מנע `sumeragi` שינה לאחר הודעה אגנוסטית לתפקיד.
- hyperledger#3046 ודא ש-Iroha יכול להתחיל בחן על ריק
  `./storage`
- hyperledger#2599 הסר מוך לחדר הילדים.
- hyperledger#3087 אסוף הצבעות ממאמתי סט B לאחר שינוי תצוגה.
- hyperledger#3056 תקן תליית `tps-dev` benchmark.
- hyperledger#1170 יישם טיפול soft-fork בסגנון שיבוט-wsv.
- hyperledger#2456 הפוך את בלוק בראשית ללא הגבלה.
- hyperledger#3038 הפעל מחדש multisigs.
- hyperledger#2894 תקן את `LOG_FILE_PATH` דה-סריאליזציה של משתנה env.
- hyperledger#2803 החזר קוד סטטוס נכון עבור שגיאות חתימה.
- hyperledger#2963 `Queue` הסר עסקאות בצורה נכונה.
- hyperledger#0000 Vergen breaking CI.
- hyperledger#2165 הסר את פידג'ט שרשרת הכלים.
- hyperledger#2506 תקן את אימות החסימה.
- hyperledger#3013 מאמתים כראוי צריבת שרשרת.
- hyperledger#2998 מחק קוד שרשרת שאינו בשימוש.
- hyperledger#2816 העבר את אחריות הגישה לבלוקים לקורה.
- hyperledger#2384 החלף פענוח ב-decode_all.
- hyperledger#1967 החלף ValueName בשם.
- hyperledger#2980 תקן ערך בלוק מסוג ffi.
- hyperledger#2858 הצג חנייה_מגרש::Mutex במקום std.
- hyperledger#2850 תקן דה-סידריאליזציה/פענוח של `Fixed`
- hyperledger#2923 החזר `FindError` כאשר `AssetDefinition` לא
  קיימים.
- hyperledger#0000 תיקון `panic_on_invalid_genesis.sh`
- hyperledger#2880 סגור חיבור websocket כראוי.
- hyperledger#2880 תקן זרימת בלוק.
- hyperledger#2804 `iroha_client_cli` שלח חסימת עסקאות.
- hyperledger#2819 העבר חברים לא חיוניים מתוך WSV.
- תקן באג הרקורסיה בהמשכת ביטויים.
- hyperledger#2834 שפר את תחביר הקיצור.
- hyperledger#2379 הוסף יכולת לזרוק בלוקי Kura חדשים ל-blocks.txt.
- hyperledger#2758 הוסף מבנה מיון לסכימה.
- CI.
- hyperledger#2548 אזהרה על קובץ בראשית גדול.
- hyperledger#2638 עדכן את `whitepaper` והפצת שינויים.
- hyperledger#2678 תקן בדיקות ב-Staging branch.
- hyperledger#2678 תיקון בדיקות מבטל את כיבוי כוח Kura.
- hyperledger#2607 Refactor של קוד sumeragi ליותר פשטות ו
  תיקוני חוסן.
- hyperledger#2561 הכנס מחדש את שינויי התצוגה לקונצנזוס.
- hyperledger#2560 הוסף חזרה ב-block_sync וניתוק עמיתים.
- hyperledger#2559 הוסף כיבוי שרשור של sumeragi.
- hyperledger#2558 אמת את הבראשית לפני עדכון ה-wsv מ-kura.
- hyperledger#2465 יישם מחדש את צומת sumeragi כמצב יחיד
  מכונה.
- hyperledger#2449 יישום ראשוני של Sumeragi Restructuring.
- hyperledger#2802 תקן את טעינת ה-env עבור תצורה.
- hyperledger#2787 הודע לכל מאזין לכיבוי בגלל פאניקה.
- hyperledger#2764 הסר מגבלה על גודל הודעה מרבי.
- #2571: עדיף Kura Inspector UX.
- hyperledger#2703 תקן באגים של Orillion dev env.
- תקן שגיאת הקלדה בהערת מסמך ב-schema/src.
- hyperledger#2716 הפוך את משך זמן פעולה לציבורי.
- hyperledger#2700 ייצוא `KURA_BLOCK_STORE_PATH` בתמונות docker.
- hyperledger#0 הסר את `/iroha/rust-toolchain.toml` מהבונה
  תמונה.
- hyperledger#0 תיקון `docker-compose-single.yml`
- hyperledger#2554 העלה שגיאה אם `secp256k1` seed קצר מ-32
  בתים.
- hyperledger#0 שנה את `test_env.sh` כדי להקצות אחסון לכל עמית.
- hyperledger#2457 סגור בכוח את הקורה בבדיקות.
- hyperledger#2623 תקן doctest עבור VariantCount.
- עדכן שגיאה צפויה בבדיקות ui_fail.
- תקן הערת מסמך שגויה במאמתי הרשאות.- hyperledger#2422 הסתר מפתחות פרטיים בתגובת נקודת הקצה לתצורה.
- hyperledger#2492: תקן לא את כל הטריגרים שמתבצעים התואמים אירוע.
- hyperledger#2504 תקן רף כושל של tps benchmark.
- hyperledger#2477 תקן באג כאשר הרשאות מתפקידים לא נספרו.
- hyperledger#2416 תקן מוך בזרוע macOS.
- hyperledger#2457 תקן בדיקות מתקלפות הקשורות לכיבוי בגלל פאניקה.
  #2457: הוסף כיבוי בתצורת פאניקה
- hyperledger#2473 לנתח rustc --גרסה במקום RUSTUP_TOOLCHAIN.
- hyperledger#1480 כבה בגלל פאניקה. #1480: הוסף וו פאניקה כדי לצאת מהתוכנית בבהלה
- hyperledger#2376 Kura מפושט, ללא אסינכרון, שני קבצים.
- hyperledger#0000 Docker כישלון בנייה.
- hyperledger#1649 הסר את `spawn` מ-`do_send`
- hyperledger#2128 תקן את הבנייה והאיטרציה של `MerkleTree`.
- hyperledger#2137 הכן בדיקות להקשר מרובה תהליכים.
- hyperledger#2227 יישם רישום ובטל רישום עבור נכס.
- hyperledger#2081 תקן באג הענקת תפקידים.
- hyperledger#2358 הוסף גרסה עם פרופיל ניפוי באגים.
- hyperledger#2294 הוסף יצירת פלמגרף ל-oneshot.rs.
- hyperledger#2202 תקן את השדה הכולל בתגובת השאילתה.
- hyperledger#2081 תקן את מקרה המבחן כדי להעניק את התפקיד.
- hyperledger#2017 תקן את ביטול הרישום של תפקידים.
- hyperledger#2303 תיקון docker-compose' עמיתים לא נסגר בחן.
- hyperledger#2295 תקן באג ההפעלה של ביטול הרישום.
- hyperledger#2282 לשפר את ה-FFI נובע מיישום getset.
- hyperledger#1149 הסר קוד nocheckin.
- hyperledger#2232 הפוך את Iroha להדפיס הודעה משמעותית כאשר בראשית יש יותר מדי isi.
- hyperledger#2170 תקן את ה-build in docker container במכונות M1.
- hyperledger#2215 הפוך לילי-2022-04-20 לאופציונלי עבור `cargo build`
- hyperledger#1990 אפשר אתחול עמית באמצעות env vars בהעדר config.json.
- hyperledger#2081 תקן רישום תפקידים.
- hyperledger#1640 צור config.json ו-genesis.json.
- hyperledger#1716 תקן כשלון קונצנזוס עם מקרים f=0.
- hyperledger#1845 ניתן להטביע נכסים שאינם ניתנים להטבעה פעם אחת בלבד.
- hyperledger#2005 תיקון `Client::listen_for_events()` לא סוגר את זרם WebSocket.
- hyperledger#1623 צור RawGenesisBlockBuilder.
- hyperledger#1917 הוסף מאקרו easy_from_str_impl.
- hyperledger#1990 אפשר אתחול עמית באמצעות env vars בהעדר config.json.
- hyperledger#2081 תקן רישום תפקידים.
- hyperledger#1640 צור config.json ו-genesis.json.
- hyperledger#1716 תקן כשלון קונצנזוס עם מקרים f=0.
- hyperledger#1845 ניתן להטביע נכסים שאינם ניתנים להטבעה פעם אחת בלבד.
- hyperledger#2005 תיקון `Client::listen_for_events()` לא סוגר את זרם WebSocket.
- hyperledger#1623 צור RawGenesisBlockBuilder.
- hyperledger#1917 הוסף מאקרו easy_from_str_impl.
- hyperledger#1922 העבר את crypto_cli לכלים.
- hyperledger#1969 הפוך את התכונה `roles` לחלק מערך התכונות המוגדר כברירת מחדל.
- hyperledger#2013 תיקון חם CLI args.
- hyperledger#1897 הסר usize/isize מהסידרה.
- hyperledger#1955 תקן אפשרות להעביר את `:` בתוך `web_login`
- hyperledger#1943 הוסף שגיאות שאילתה לסכימה.
- hyperledger#1939 תכונות מתאימות עבור `iroha_config_derive`.
- hyperledger#1908 תיקון טיפול בערך אפס עבור סקריפט ניתוח טלמטריה.
- hyperledger#0000 הפוך את doc-test להתעלמות מפורשות.
- hyperledger#1848 מנע צריבה של מפתחות ציבוריים לכלום.
- hyperledger#1811 הוסיף בדיקות ובדיקות לביטול מפתחות עמיתים מהימנים.
- hyperledger#1821 הוסף IntoSchema עבור MerkleTree ו-VersionedValidBlock, תקן סכימות HashOf ו-SignatureOf.- hyperledger#1819 הסר עקבות מדוח שגיאה באימות.
- hyperledger#1774 יומן סיבה מדויקת לכשלי אימות.
- hyperledger#1714 השווה PeerId רק לפי מפתח.
- hyperledger#1788 צמצם את טביעת הזיכרון של `Value`.
- hyperledger#1804 תיקון סכימה עבור HashOf, SignatureOf, הוסף בדיקה כדי לוודא שלא חסרות סכמות.
- hyperledger#1802 שיפורים בקריאות ביומן.
  - יומן אירועים הועבר לרמת מעקב
  - ctx הוסר מלכידת יומן
  - צבעי הטרמינל נעשים אופציונליים (לצורך פלט יומן טוב יותר לקבצים)
- hyperledger#1783 מדד torii קבוע.
- hyperledger#1772 תיקון לאחר #1764.
- hyperledger#1755 תיקונים קלים עבור #1743, #1725.
  - תקן JSONs לפי #1743 `Domain` שינוי מבנה
- hyperledger#1751 תיקוני קונצנזוס. #1715: תיקוני קונצנזוס לטיפול בעומס גבוה (#1746)
  - הצג תיקוני טיפול בשינויים
  - הצג הוכחות לשינוי שנעשו ללא תלות ב-hash של עסקאות מסוימות
  - העברת הודעות מופחתת
  - אסוף הצבעות לשינוי צפייה במקום לשלוח הודעות מיד (משפר את עמידות הרשת)
  - שימוש מלא ב-Actor Framework ב-Sumeragi (תזמן הודעות לעצמי במקום התחלת משימות)
  - משפר את הזרקת תקלות לבדיקות עם Sumeragi
  - מקרב את קוד הבדיקה לקוד הייצור
  - מסיר עטיפות מסובכות מדי
  - מאפשר ל-Sumeragi להשתמש בהקשר של שחקן בקוד הבדיקה
- hyperledger#1734 עדכן את genesis כך שיתאים לאימות הדומיין החדש.
- hyperledger#1742 שגיאות קונקרטיות שהוחזרו בהוראות `core`.
- hyperledger#1404 אימות תוקן.
- hyperledger#1636 הסר את `trusted_peers.json` ו-`structopt`
  #1636: הסר את `trusted_peers.json`.
- hyperledger#1706 עדכון `max_faults` עם עדכון טופולוגיה.
- hyperledger#1698 תיקנו מפתחות ציבוריים, תיעוד והודעות שגיאה.
- גליונות טביעה (1593 ו-1405) גיליון 1405

### Refactor- חלץ פונקציות מהלולאה הראשית של sumeragi.
- Refactor `ProofChain` לסוג חדש.
- הסר את `Mutex` מ-`Metrics`
- הסר את התכונה הלילית של adt_const_generics.
- hyperledger#3039 הצג מאגר המתנה עבור ה-multisigs.
- פשט sumeragi.
- hyperledger#3053 תקן מוך קליפי.
- hyperledger#2506 הוסף בדיקות נוספות על אימות בלוקים.
- הסר את `BlockStoreTrait` ב-Kura.
- עדכון מוך עבור `nightly-2022-12-22`
- hyperledger#3022 הסר `Option` ב-`transaction_cache`
- hyperledger#3008 הוסף ערך נישה ל-`Hash`
- עדכן את המוך ל-1.65.
- הוסף בדיקות קטנות כדי להגביר את הכיסוי.
- הסר קוד מת מ-`FaultInjection`
- התקשר ל-p2p בתדירות נמוכה יותר מ-sumeragi.
- hyperledger#2675 אמת שמות/מזהים של פריטים מבלי להקצות Vec.
- hyperledger#2974 מנע זיוף בלוקים ללא אימות מלא.
- יעיל יותר `NonEmpty` בקומבינטורים.
- hyperledger#2955 הסר חסימה מהודעה בלוק חתום.
- hyperledger#1868 מנע שליחת עסקאות מאומתות
  בין עמיתים.
- hyperledger#2458 הטמעת API של קומבינטור גנרי.
- הוסף תיקיית אחסון לתוך gitignore.
- hyperledger#2909 יציאות קוד קשיח עבור הבאות.
- hyperledger#2747 שנה את `LoadFromEnv` API.
- שפר את הודעות השגיאה על כשל בתצורה.
- הוסף דוגמאות נוספות ל-`genesis.json`
- הסר תלות שאינה בשימוש לפני שחרור `rc9`.
- סיום מוך ב-Sumeragi החדש.
- חלץ נהלי משנה בלולאה הראשית.
- hyperledger#2774 שנה את `kagami` מצב יצירת בראשית מדגל ל
  פקודה משנה.
- hyperledger#2478 הוסף `SignedTransaction`
- hyperledger#2649 הסר את ארגז `byteorder` מ-`Kura`
- שנה את שם `DEFAULT_BLOCK_STORE_PATH` מ-`./blocks` ל-`./storage`
- hyperledger#2650 הוסף את `ThreadHandler` לכיבוי תת-מודולי iroha.
- hyperledger#2482 אחסן אסימוני הרשאות `Account` ב-`Wsv`
- הוסף מוך חדש ל-1.62.
- שפר את הודעות השגיאה של `p2p`.
- hyperledger#2001 `EvaluatesTo` בדיקת סוג סטטי.
- hyperledger#2052 הפוך אסימוני הרשאות לרישום עם הגדרה.
  #2052: יישם PermissionTokenDefinition
- ודא שכל שילובי התכונות עובדים.
- hyperledger#2468 הסר על תכונת ניפוי באגים ממאמתי ההרשאות.
- hyperledger#2419 הסר `drop`s מפורש.
- hyperledger#2253 הוסף תכונה `Registrable` ל-`data_model`
- יישם `Origin` במקום `Identifiable` עבור אירועי הנתונים.
- hyperledger#2369 מאמת הרשאות Refactor.
- hyperledger#2307 הפוך את `events_sender` ב-`WorldStateView` ללא אופציונלי.
- hyperledger#1985 הקטנת גודל של מבנה `Name`.
- הוסף עוד `const fn`.
- בצע בדיקות אינטגרציה באמצעות `default_permissions()`
- הוסף עטיפות של אסימוני הרשאה ב-private_blockchain.
- hyperledger#2292 הסר `WorldTrait`, הסר כללי מ-`IsAllowedBoxed`
- hyperledger#2204 הפוך פעולות הקשורות לנכסים לגנריות.
- hyperledger#2233 החלף את `impl` ב-`derive` עבור `Display` ו-`Debug`.
- שיפורי מבנה שניתן לזהות.
- hyperledger#2323 הודעת שגיאה של שיפור kura init.
- hyperledger#2238 הוסף בונה עמיתים לבדיקות.
- hyperledger#2011 פרמטרי תצורה תיאוריים נוספים.
- hyperledger#1896 פשט את יישום `produce_event`.
- Refactor סביב `QueryError`.
- העבר את `TriggerSet` ל-`data_model`.
- צד `WebSocket` של לקוח hyperledger#2145 refactor, חילוץ לוגיקה נתונים טהורה.
- הסר תכונה `ValueMarker`.
- hyperledger#2149 חשוף את `Mintable` ו-`MintabilityError` ב-`prelude`
- hyperledger#2144 עצב מחדש את זרימת העבודה http של הלקוח, חשוף ממשק API פנימי.- העבר ל-`clap`.
- צור `iroha_gen` בינארי, איחוד מסמכים, schema_bin.
- hyperledger#2109 הפוך את מבחן `integration::events::pipeline` ליציב.
- hyperledger#1982 מקיף גישה למבני `iroha_crypto`.
- הוסף את בונה `AssetDefinition`.
- הסר את `&mut` המיותר מהממשק ה-API.
- להטמיע גישה למבני מודל נתונים.
- hyperledger#2144 עצב מחדש את זרימת העבודה http של הלקוח, חשוף ממשק API פנימי.
- העבר ל-`clap`.
- צור `iroha_gen` בינארי, איחוד מסמכים, schema_bin.
- hyperledger#2109 הפוך את מבחן `integration::events::pipeline` ליציב.
- hyperledger#1982 עוטף גישה למבני `iroha_crypto`.
- הוסף את בונה `AssetDefinition`.
- הסר את `&mut` המיותר מהממשק ה-API.
- להטמיע גישה למבני מודל נתונים.
- Core, `sumeragi`, פונקציות מופע, `torii`
- hyperledger#1903 העבר פליטת אירועים לשיטות `modify_*`.
- פיצול קובץ `data_model` lib.rs.
- הוסף הפניה ל-wsv לתור.
- hyperledger#1210 פיצול זרם אירועים.
  - העבר פונקציונליות הקשורה לעסקה למודול data_model/transaction
- hyperledger#1725 הסר מצב גלובלי ב-Torii.
  - יישם את `add_state macro_rules` והסר את `ToriiState`
- תקן שגיאת linter.
- hyperledger#1661 `Cargo.toml` ניקוי.
  - למיין את התלות במטען
- hyperledger#1650 לסדר את `data_model`
  - העבר את העולם ל-wsv, תקן תפקידים, הפק את IntoSchema עבור CommittedBlock
- ארגון קבצי `json` ו-readme. עדכן את Readme כדי להתאים לתבנית.
- 1529: רישום מובנה.
  - הודעות יומן Refactor
- `iroha_p2p`
  - הוסף הפרטת p2p.

### תיעוד

- עדכון Iroha Client CLI readme.
- עדכון קטעי הדרכה.
- הוסף 'סort_by_metadata_key' למפרט API.
- עדכון קישורים לתיעוד.
- הרחב את המדריך עם מסמכים הקשורים לנכסים.
- הסר קובצי מסמך מיושנים.
- סקירת סימני פיסוק.
- העבר כמה מסמכים למאגר ההדרכה.
- דוח התקלפות לסניף הבמה.
- צור יומן שינויים עבור pre-rc.7.
- דוח רעידות ל-30 ביולי.
- גרסאות Bump.
- עדכן את התקלפות הבדיקה.
- hyperledger#2499 תקן הודעות שגיאה client_cli.
- hyperledger#2344 צור CHANGELOG עבור 2.0.0-pre-rc.5-lts.
- הוסף קישורים להדרכה.
- עדכן מידע על git hooks.
- כתיבת מבחן התקלפות.
- hyperledger#2193 עדכון תיעוד לקוח Iroha.
- hyperledger#2193 עדכון תיעוד Iroha CLI.
- hyperledger#2193 עדכן README עבור ארגז מאקרו.
 - hyperledger#2193 עדכון תיעוד כלי המפענח של Norito.
- hyperledger#2193 עדכון תיעוד Kagami.
- hyperledger#2193 עדכן תיעוד של נקודות מידה.
- hyperledger#2192 עיין בהנחיות התורמות.
- תקן הפניות שבורות בקוד.
- hyperledger#1280 מדדי Iroha של מסמך.
- hyperledger#2119 הוסף הנחיות כיצד לטעון מחדש את Iroha במיכל Docker.
- hyperledger#2181 סקירת README.
- hyperledger#2113 תכונות מסמך בקבצי Cargo.toml.
- hyperledger#2177 נקה פלט gitchangelog.
- hyperledger#1991 הוסף את ה-readme למפקח Kura.
- hyperledger#2119 הוסף הנחיות כיצד לטעון מחדש את Iroha במיכל Docker.
- hyperledger#2181 סקירת README.
- hyperledger#2113 תכונות מסמך בקבצי Cargo.toml.
- hyperledger#2177 נקה פלט gitchangelog.
- hyperledger#1991 הוסף את ה-readme למפקח Kura.
- צור יומן שינויים אחרונים.
- יצירת יומן שינויים.
- עדכן קבצי README מיושנים.
- הוספת מסמכים חסרים ל-`api_spec.md`.

### שינויים ב-CI/CD- הוסף עוד חמישה רצים שמתארחים בעצמם.
- הוסף תג תמונה רגיל עבור הרישום Soramitsu.
- דרך לעקיפת הבעיה עבור libgit2-sys 0.5.0. חזור ל-0.4.4.
- נסה להשתמש בתמונה מבוססת קשת.
- עדכן זרימות עבודה כדי לעבוד על מיכל חדש ללילה בלבד.
- הסר נקודות כניסה בינאריות מהכיסוי.
- החלף מבחני פיתוח לרצים באירוח עצמי של Equinix.
- hyperledger#2865 הסר את השימוש בקובץ tmp מ-`scripts/check.sh`
- hyperledger#2781 הוסף קיזוז כיסוי.
- השבת מבחני אינטגרציה איטיים.
- החלף את תמונת הבסיס ב-docker cache.
- hyperledger#2781 הוסף תכונת אב של codecov commit.
- העבר עבודות ל-github runners.
- hyperledger#2778 בדיקת תצורת לקוח.
- hyperledger#2732 הוסף תנאים לעדכון תמונות מבוססות iroha2 והוספה
  תוויות יחסי ציבור.
- תקן בניית תמונה לילית.
- תקן שגיאת `buildx` עם `docker/build-push-action`
- עזרה ראשונה לא מתפקד `tj-actions/changed-files`
- אפשר פרסום רציף של תמונות, אחרי #2662.
- הוסף רישום נמל.
- תווית אוטומטית `api-changes` ו-`config-changes`
- בצע hash בתמונה, קובץ כלים שוב, בידוד ממשק משתמש,
  מעקב אחר סכימה.
- הפוך את זרימות העבודה של פרסום לרצף ומשלים ל-#2427.
- hyperledger#2309: הפעל מחדש בדיקות doc ב-CI.
- hyperledger#2165 הסר את התקנת codecov.
- העבר למיכל חדש כדי למנוע התנגשויות עם המשתמשים הנוכחיים.
 - hyperledger#2158 שדרוג `parity_scale_codec` ותלויות אחרות. (Codec Norito)
- תקן את המבנה.
- hyperledger#2461 שפר את iroha2 CI.
- עדכון `syn`.
- העבר את הכיסוי לזרימת עבודה חדשה.
- גרסה הפוכה לכניסה לדוקר.
- הסר את מפרט הגרסה של `archlinux:base-devel`
- עדכן שימוש חוזר בדוחות Dockerfiles ודוחות Codecov ובמקבילות.
- יצירת יומן שינויים.
- הוסף קובץ `cargo deny`.
- הוסף ענף `iroha2-lts` עם זרימת עבודה שהועתקה מ-`iroha2`
- hyperledger#2393 חבט בגרסה של תמונת הבסיס Docker.
- hyperledger#1658 הוסף בדיקת תיעוד.
- בליטת גרסה של ארגזים והסר תלות שאינה בשימוש.
- הסר דיווחי כיסוי מיותרים.
- hyperledger#2222 פיצול בדיקות לפי האם זה כולל כיסוי או לא.
- hyperledger#2153 תיקון מס' 2154.
- גרסה לחבוט בכל הארגזים.
- תקן את צינור הפריסה.
- hyperledger#2153 תקן כיסוי.
- הוסף בדיקת בראשית ועדכן תיעוד.
- בליטת חלודה, עובש ולילה ל-1.60, 1.2.0 ו-1.62 בהתאמה.
- load-rs טריגרים.
- hyperledger#2153 תיקון מס' 2154.
- גרסה לחבוט בכל הארגזים.
- תקן את צינור הפריסה.
- hyperledger#2153 תקן כיסוי.
- הוסף בדיקת בראשית ועדכן תיעוד.
- בליטת חלודה, עובש ולילה ל-1.60, 1.2.0 ו-1.62 בהתאמה.
- load-rs טריגרים.
- load-rs:שחרר טריגרים של זרימת עבודה.
- תקן את זרימת העבודה בדחיפה.
- הוסף טלמטריה לתכונות ברירת המחדל.
- הוסף תג מתאים כדי לדחוף את זרימת העבודה הראשית.
- לתקן מבחנים כושלים.
- hyperledger#1657 עדכן את התמונה לחלודה 1.57. #1630: חזור לרצים שמתארחים בעצמם.
- שיפורי CI.
- החלפת כיסוי לשימוש `lld`.
- תיקון תלות CI.
- שיפורים בפילוח CI.
- משתמש בגרסת Rust קבועה ב-CI.
- תקן את Docker פרסום ו- iroha2-dev push CI. העבר את הסיקור והספסל ליחסי ציבור
- הסר את בדיקת Iroha מלאה מיותרת ב-CI docker.

  המבנה Iroha הפך לחסר תועלת מכיוון שהוא נעשה כעת בתמונת docker עצמה. אז ה-CI בונה רק את ה-cli של הלקוח המשמש בבדיקות.
- הוסף תמיכה בסניף iroha2 בצנרת CI.
  - בדיקות ארוכות רצו רק ב-PR לתוך iroha2
  - פרסם תמונות Docker רק מ-iroha2
- מטמוני CI נוספים.

### חיבור אינטרנט


### בליטות בגרסה- גרסה ל-pre-rc.13.
- גרסה ל-pre-rc.11.
- גרסה ל-RC.9.
- גרסה ל-RC.8.
- עדכן גרסאות ל-RC7.
- הכנות לפני שחרור.
- עדכן את Mould 1.0.
- תלות בבליטה.
- עדכון api_spec.md: תקן גופי בקשה/תגובה.
- עדכון גרסת חלודה ל-1.56.0.
- עדכן את המדריך התורם.
- עדכן את README.md ואת `iroha/config.json` כדי להתאים לפורמט API וכתובת URL חדשים.
- עדכן יעד פרסום Docker ל-Hyperledger/iroha2 #1453.
- מעדכן את זרימת העבודה כך שתתאים לראשי.
- עדכן את מפרט ה-API ותקן את נקודת הקצה הבריאותית.
- עדכון חלודה ל-1.54.
- Docs(iroha_crypto): עדכן `Signature` מסמכים ויישור ארג'ים של `verify`
- חבטה בגרסת Ursa מ-0.3.5 ל-0.3.6.
- עדכן זרימות עבודה לרצים חדשים.
- עדכן dockerfile עבור אחסון במטמון ובניית ci מהירות יותר.
- עדכון גרסת libssl.
- עדכן dockerfiles ו-async-std.
- תקן clippy מעודכן.
- עדכון מבנה הנכס.
  - תמיכה בהוראות ערך מפתח בנכס
  - סוגי נכסים כרשיון
  - פגיעות הצפה בתיקון ISI של נכסים
- מדריך תורם עדכונים.
- עדכון לא מעודכן lib.
- עדכן נייר לבן ותקן בעיות מוך.
- עדכן את cucumber_rust lib.
- עדכוני README ליצירת מפתחות.
- עדכן את זרימות העבודה של Github Actions.
- עדכן את זרימות העבודה של Github Actions.
- עדכון requirements.txt.
- עדכון common.yaml.
- עדכוני מסמכים מאת שרה.
- עדכון לוגיקה של הוראות.
- עדכן נייר לבן.
- עדכון תיאור פונקציות הרשת.
- עדכן נייר לבן על סמך הערות.
- הפרדת עדכון WSV והגירה ל-Scale.
- עדכן את gitignore.
- עדכן מעט את התיאור של הקורה ב-WP.
- עדכן את תיאור הקורה בנייר לבן.

### סכימה

- hyperledger#2114 תמיכה באוספים ממוינים בסכמות.
- hyperledger#2108 הוסף עימוד.
- hyperledger#2114 תמיכה באוספים ממוינים בסכמות.
- hyperledger#2108 הוסף עימוד.
- הפוך את הסכימה, הגרסה והמאקרו no_std לתואמים.
- תקן חתימות בסכימה.
- ייצוג שונה של `FixedPoint` בסכימה.
- נוסף `RawGenesisBlock` לבדיקת סכימה פנימית.
- שינו מודלים של אובייקטים ליצירת סכימה IR-115.

### מבחנים

- hyperledger#2544 בדיקות הדרכה.
- hyperledger#2272 הוסף בדיקות עבור שאילתת 'FindAssetDefinitionById'.
- הוסף מבחני אינטגרציה של `roles`.
- תקן את פורמט מבחני ה-UI, העבר את בדיקות ה-Ui לגזירת ארגזים.
- תקן בדיקות מדומה (באג לא מסודר עתידי).
- הסירו את ארגז ה-DSL והעבירו את הבדיקות ל-`data_model`
- ודא שבדיקות רשת לא יציבות עוברות קוד חוקי.
- נוספו בדיקות ל-iroha_p2p.
- לוכד יומנים בבדיקות אלא אם הבדיקה נכשלת.
- הוסף סקר לבדיקות ותקן בדיקות שבר לעתים נדירות.
- בודק התקנה מקבילה.
- הסר שורש מבדיקות iroha init ו- iroha_client.
- תקן אזהרות דגימות של בדיקות ומוסיף בדיקות ל-ci.
- תקן שגיאות אימות `tx` במהלך בדיקות השוואת ביצועים.
- hyperledger#860: Iroha שאילתות ובדיקות.
- מדריך ISI מותאם אישית Iroha ובדיקות מלפפון.
- הוסף בדיקות עבור לקוח ללא תקן.
- שינויים ברישום גשר ובדיקות.
- בדיקות קונצנזוס עם דוגמת רשת.
- שימוש ב-temp dir לביצוע בדיקות.
- ספסלים בודקים מקרים חיוביים.
- פונקציונליות ראשונית של עץ Merkle עם בדיקות.
- בדיקות קבועות ואתחול World State View.

### אחר- העבר פרמטריזציה לתכונות והסר סוגי FFI IR.
- הוסף תמיכה באיגודים, הצג את `non_robust_ref_mut` * הטמעת המרת FFI של conststring.
- שפר את IdOrdEqHash.
- הסר את FilterOpt::BySome מ-(דה-)סריאליזציה.
- הפוך לא שקוף.
- הפוך את ContextValue לשקוף.
- הפוך את Expression::Raw tag לאופציונלי.
- הוסף שקיפות לכמה הוראות.
- שפר (דה-)סריאליזציה של RoleId.
- שפר (דה-)סריאליזציה של validator::Id.
- שפר (דה-)סריאליזציה של PermissionTokenId.
- שפר (דה-)סריאליזציה של TriggerId.
- שפר (דה-)סריאליזציה של מזהי נכסים (-הגדרה).
- שפר (דה-)סריאליזציה של AccountId.
- שפר (דה-)סריאליזציה של Ipfs ו-DomainId.
- הסר את תצורת לוגר מתצורת הלקוח.
- הוסף תמיכה למבנים שקופים ב-FFI.
- שחזר &אפשרות<T> לאפשרות<&T>
- תקן אזהרות קפיציות.
- הוסף פרטים נוספים בתיאור השגיאה `Find`.
- תקן יישומי `PartialOrd` ו-`Ord`.
- השתמש ב-`rustfmt` במקום ב-`cargo fmt`
- הסר את תכונת `roles`.
- השתמש ב-`rustfmt` במקום ב-`cargo fmt`
- שתף את workdir כאמצעי אחסון עם מופעי dev docker.
- הסר את הסוג המשויך ל-Diff ב-Execute.
- השתמש בקידוד מותאם אישית במקום החזר מרובה.
- הסר את serde_json כתלות ב-iroha_crypto.
- אפשר רק שדות ידועים בתכונת הגרסה.
- הבהרת יציאות שונות עבור נקודות קצה.
- הסר את נגזרת `Io`.
- תיעוד ראשוני של זוגות_מפתחות.
- חזור לרצים שמתארחים בעצמם.
- תקן מוכים חדשים בקוד.
- הסר את i1i1 מהתחזוקה.
- הוסף מסמך שחקן ותיקונים קלים.
- סקר במקום לדחוף את הבלוקים האחרונים.
- אירועי סטטוס עסקה נבדקו עבור כל אחד מ-7 עמיתים.
- `FuturesUnordered` במקום `join_all`
- עבור ל-GitHub Runners.
- השתמש ב-VersionedQueryResult לעומת QueryResult עבור נקודת הקצה /query.
- חבר מחדש את הטלמטריה.
- תקן את תצורת dependabot.
- הוסף commit-msg git hook כדי לכלול חתימה.
- תקן את צינור הדחיפה.
- שדרוג תלוי בוט.
- זיהוי חותמת זמן עתידית בדחיפה בתור.
- hyperledger#1197: Kura מטפל בשגיאות.
- הוסף הוראת ביטול רישום של עמיתים.
- הוסף אי-הודעה אופציונלית כדי להבחין בין עסקאות. סגור מס' 1493.
- הוסר `sudo` המיותר.
- מטא נתונים עבור דומיינים.
- תקן את ההקפצות האקראיות בזרימת העבודה `create-docker`.
- נוסף `buildx` כפי שהוצע על ידי הצינור הכושל.
- hyperledger#1454: תקן תגובת שגיאת שאילתה עם קוד סטטוס ספציפי ורמזים.
- hyperledger#1533: מצא עסקה לפי hash.
- תקן את נקודת הקצה `configure`.
- הוסף בדיקת יציבות נכסים מבוססי בוליאנית.
- הוספת פרימיטיבים קריפטו מוקלדים והגירה להצפנה בטוחה לסוגים.
- שיפורים ברישום.
- hyperledger#1458: הוסף גודל ערוץ שחקן להגדרה כ-`mailbox`.
- hyperledger#1451: הוסף אזהרה לגבי תצורה שגויה אם `faulty_peers = 0` ו-`trusted peers count > 1`
- הוסף מטפל לקבלת hash בלוק ספציפי.
- נוספה שאילתה חדשה FindTransactionByHash.
- hyperledger#1185: שנה את השם והנתיב של ארגזים.
- תקן יומנים ושיפורים כלליים.
- hyperledger#1150: קבץ 1000 בלוקים בכל קובץ
- מבחן מאמץ בתור.
- תיקון רמת יומן.
- הוסף מפרט כותרות לספריית הלקוח.
- תיקון כשל בפאניקה בתור.
- תור לתיקון.
- תיקון בניית שחרור dockerfile.
- תיקון לקוח Https.
- Speedup ci.
- 1. הסירו את כל התלות ב-ursa, למעט iroha_crypto.
- תקן הצפה בעת הפחתת משכי זמן.
- הפוך שדות לציבוריים בלקוח.
- דחף את Iroha2 ל- Dockerhub כל לילה.
- תקן קודי מצב http.
- החלף iroha_error בשגיאה זו, eyre ו-color-eyre.
- תור מחליף עם קרן צולבת אחת.- הסר כמה קצבאות מוך חסרות תועלת.
- מציג מטא נתונים עבור הגדרות נכסים.
- הסרת ארגומנטים מארגז test_network.
- הסר תלות מיותרת.
- תקן iroha_client_cli::events.
- hyperledger#1382: הסר יישום רשת ישן.
- hyperledger#1169: דיוק נוסף עבור נכסים.
- שיפורים בהפעלת עמיתים:
  - מאפשר טעינת מפתח ציבורי Genesis רק מ-env
  - כעת ניתן לציין config, genesis ו-trusted_peers נתיב בפרמטרים של cli
- hyperledger#1134: אינטגרציה של Iroha P2P.
- שנה את נקודת הקצה של השאילתה ל-POST במקום ל-GET.
- בצע את on_start בשחקן באופן סינכרוני.
- נדידה לעיוות.
- עבד מחדש את ההתחייבות עם תיקוני באגים של ברוקר.
- חזור ל"מציג מספר תיקוני מתווך" commit(9c148c33826067585b5868d297dcdd17c0efe246)
- מציג תיקוני מתווך מרובים:
  - בטל את המנוי מהברוקר בעצירת השחקן
  - תמיכה במספר מנויים מאותו סוג שחקן (בעבר TODO)
  - תקן באג שבו הברוקר תמיד שם את עצמי כמזהה שחקן.
- באג ברוקר (חלון ראווה לבדיקה).
- הוסף נגזרות למודל נתונים.
- הסר את rwlock מ-torii.
- בדיקות הרשאות שאילתות OOB.
- hyperledger#1272: יישום ספירת עמיתים,
- בדיקה רקורסיבית של הרשאות שאילתה בתוך ההוראות.
- תזמן שחקני עצירה.
- hyperledger#1165: יישום ספירת עמיתים.
- בדוק הרשאות שאילתה לפי חשבון בנקודת הקצה torii.
- הוסר חשיפת שימוש במעבד וזיכרון במדדי מערכת.
 - החלף את JSON ב-Norito עבור הודעות WS.
- אחסן הוכחות לשינויים בתצוגה.
- hyperledger#1168: רישום רישום נוסף אם העסקה לא עברה את תנאי בדיקת החתימה.
- תיקנו בעיות קטנות, הוספת קוד האזנה לחיבור.
- הצג את בונה טופולוגיית הרשת.
- הטמעת רשת P2P עבור Iroha.
- מוסיף מדד גודל בלוק.
- שם תכונת PermissionValidator שונה ל-IsAllowed. ושינויי שמות נוספים בהתאם
- תיקוני שקע אינטרנט במפרט API.
- מסיר תלות מיותרת מתמונת docker.
- Fmt משתמש ב-Crate import_granularity.
- מציגה את אימות ההרשאות הגנרי.
- הגירה למסגרת השחקנים.
- שנה את עיצוב הברוקר והוסף קצת פונקציונליות לשחקנים.
- מגדיר בדיקות סטטוס Codecov.
- משתמש בכיסוי מבוסס מקור עם grcov.
- תוקן פורמט Build-Args מרובים ו-ARG הוכרז מחדש עבור מיכלי בנייה ביניים.
- מציג את הודעת SubscriptionAccepted.
- הסר נכסים בשווי אפס מהחשבונות לאחר הפעלתם.
- פורמט ארגומנטים של בניית docker תוקן.
- תוקנה הודעת שגיאה אם ​​חסימת הילד לא נמצאה.
- הוסף OpenSSL מוכר לבנייה, מתקן את התלות ב-pkg-config.
- תקן את שם המאגר עבור dockerhub ו-Diff Diff.
- נוספו טקסט שגיאה ברור ושם קובץ אם לא ניתן היה לטעון TrustedPeers.
- ישויות טקסט שינו לקישורים במסמכים.
- תקן סוד שם משתמש שגוי בפרסום Docker.
- תקן שגיאות הקלדה קטנות בנייר לבן.
- מאפשר שימוש ב-mod.rs למבנה קבצים טוב יותר.
- העבר את ה-main.rs לתוך ארגז נפרד ועשה הרשאות לבלוקצ'יין ציבורי.
- הוסף שאילתות בתוך לקוח קלי.
- נדדו ממחיאות כפיים ל-structopts עבור cli.
- הגבל את הטלמטריה לבדיקת רשת לא יציבה.
- העבר תכונות למודול חוזים חכמים.
- Sed -i "s/world_state_view/wsv/g"
- העבר חוזים חכמים למודול נפרד.
- תיקון באגים באורך תוכן רשת Iroha.
- מוסיף אחסון מקומי למשימה עבור מזהה שחקן. שימושי לזיהוי מבוי סתום.
- הוסף בדיקת זיהוי מבוי סתום ל-CI
- הוסף מאקרו Introspect.
- מבלבל שמות של זרימת עבודה גם תיקוני עיצוב
- שינוי ממשק ה-API של השאילתה.
- הגירה מ-async-std לטוקיו.
- הוסף ניתוח של טלמטריה ל-ci.- הוסף טלמטריה עתידית עבור iroha.
- הוסף עתידי iroha לכל פונקציית אסינכרון.
- הוסף עתידי iroha לצפייה במספר הסקרים.
- פריסה ותצורה ידנית נוספה ל-README.
- תיקון כתב.
- הוסף מאקרו של נגזרת הודעה.
- הוסף מסגרת שחקן פשוטה.
- הוסף תצורת dependabot.
- הוסף כתבי פאניקה וטעויות נחמדים.
- העברת גרסת חלודה ל-1.52.1 ותיקונים מתאימים.
- משימות אינטנסיביות של מעבד חוסם השרצים בשרשורים נפרדים.
- השתמש ב- unique_port וב-cargo-lints מ-crates.io.
- תיקון עבור WSV ללא נעילה:
  - מסיר Dashmaps ומנעולים מיותרים ב-API
  - מתקן באג עם מספר מוגזם של בלוקים שנוצרו (עסקאות שנדחו לא נרשמו)
  - מציג את סיבת השגיאה המלאה לשגיאות
- הוסף מנוי טלמטריה.
- שאילתות לתפקידים והרשאות.
- העבר בלוקים מקורה ל-wsv.
- שנה למבני נתונים ללא נעילה בתוך wsv.
- תיקון פסק זמן ברשת.
- תיקון נקודת קצה בריאותית.
- מציג תפקידים.
- הוסף תמונות דחיפה מסניף dev.
- הוסף מוך אגרסיבי יותר והסר פאניקות מהקוד.
- עיבוד מחדש של תכונת הביצוע לקבלת הוראות.
- הסר קוד ישן מ- iroha_config.
- IR-1060 מוסיף בדיקות הענק עבור כל ההרשאות הקיימות.
- תקן ulimit וזמן קצוב עבור iroha_network.
- תיקון בדיקת פסק זמן Ci.
- הסר את כל הנכסים כשההגדרה שלהם הוסרה.
- תקן פאניקה של wsv בהוספת נכס.
- הסר את Arc ו-Rwlock עבור ערוצים.
- תיקון רשת Iroha.
- מאמתי הרשאות משתמשים בהפניות בהמחאות.
- הוראת מענק.
- נוספה תצורה עבור מגבלות אורך מחרוזת ואימות של מזהים עבור NewAccount, Domain ו-AssetDefinition IR-1036.
- החלף יומן עם מעקב lib.
- הוסף ci check for docs והכחיש מאקרו dbg.
- מציג הרשאות שניתנות להענקה.
- הוסף ארגז iroha_config.
- הוסף את @alerdenisov כבעל קוד כדי לאשר את כל בקשות המיזוג הנכנסות.
- תיקון בדיקת גודל העסקה במהלך קונצנזוס.
- החזר שדרוג של async-std.
- החלף כמה קונסטסטים בעוצמה של 2 IR-1035.
- הוסף שאילתה כדי לאחזר היסטוריית עסקאות IR-1024.
- הוסף אימות הרשאות לאחסון ומבנה מחדש של מאמתי הרשאות.
- הוסף חשבון חדש לרישום חשבון.
- הוסף סוגים להגדרת הנכס.
- מציג מגבלות מטא נתונים הניתנות להגדרה.
- מציג מטא נתונים של עסקאות.
- הוסף ביטויים בתוך שאילתות.
- הוסף lints.toml ותקן אזהרות.
- הפרד Trusted_peers מ-config.json.
- תקן שגיאת הקלדה ב-URL לקהילת Iroha 2 בטלגרם.
- תקן אזהרות קפיציות.
- מציג תמיכה במטא נתונים של ערך מפתח עבור חשבון.
- הוסף ניהול גרסאות של בלוקים.
- Fixup ci linting חזרות.
- הוסף ביטויים mul,div,mod,raise_to.
- הוסף into_v* לניהול גרסאות.
- החלף את Error::msg במאקרו שגיאה.
- כתוב מחדש iroha_http_server ועבד מחדש שגיאות Torii.
 - משדרג את גרסת Norito ל-2.
- תיאור גירסת נייר לבן.
- עימוד בלתי ניתן לטעות. תקן את המקרים שבהם עימוד עשוי להיות מיותר באמצעות שגיאות, ולא מחזיר אוספים ריקים במקום זאת.
- הוסף נגזר (שגיאה) עבור enums.
- תקן גרסה לילית.
- הוסף ארגז iroha_error.
- הודעות מנוסחות.
- מציג פרימיטיבים לגירסת מיכל.
- תקן אמות מידה.
- הוסף עימוד.
- הוסף פענוח קידוד varint.
- שנה חותמת זמן של שאילתה ל-u128.
- הוסף את Enum RejectionReason עבור אירועי צינור.
- מסיר שורות מיושנות מקבצי בראשית. היעד הוסר מרישום ISI בהתחייבויות קודמות.
- מפשט את הרישום והביטול של ISIs.
- תקן פסק זמן להתחייבות שלא נשלח ברשת 4 עמיתים.
- ערבוב טופולוגי בעת שינוי תצוגת.- הוסף מיכלים אחרים עבור מאקרו נגזר FromVariant.
- הוסף תמיכת MST עבור לקוח קלי.
- הוסף מאקרו FromVariant ובסיס קוד ניקוי.
- הוסף i1i1 לבעלי קוד.
- עסקאות רכילות.
- הוסף אורך להוראות וביטויים.
- הוסף מסמכים לחסימת זמן ופרמטרי זמן.
- החליף את תכונות אימות וקבל ב-TryFrom.
- הציגו המתנה רק למספר המינימלי של עמיתים.
- הוסף פעולת github לבדיקת API עם iroha2-java.
- הוסף בראשית עבור docker-compose-single.yml.
- מצב ברירת המחדל של בדיקת חתימה לחשבון.
- הוסף בדיקה לחשבון עם מספר חותמים.
- הוסף תמיכת לקוח API עבור MST.
- בניית docker.
- הוסף בראשית ל-docker compose.
- הצג MST מותנה.
- הוסף wait_for_active_peers impl.
- הוסף בדיקה עבור לקוח isahc בשרת iroha_http_.
- מפרט API של הלקוח.
- ביצוע שאילתה בביטויים.
- משלב ביטויים ו-ISIs.
- ביטויים עבור ISI.
- תקן את מדדי תצורת החשבון.
- הוסף תצורת חשבון עבור הלקוח.
- תקן את `submit_blocking`.
- נשלחים אירועי צנרת.
- חיבור שקע אינטרנט של לקוח Iroha.
- הפרדת אירועים לאירועי צינור ונתונים.
- בדיקת אינטגרציה להרשאות.
- הוסף בדיקות הרשאה עבור צריבה ונענע.
- בטל את הרישום של הרשאת ISI.
- תקן אמות מידה עבור יחסי ציבור במבנה העולמי.
- הצג את מבנה העולם.
- הטמעת רכיב טעינת בלוק בראשית.
- הצג חשבון בראשית.
- הצג את בונה מאמת ההרשאות.
- הוסף תוויות ל-Iroha2 PRs עם Github Actions.
- הצג את מסגרת ההרשאות.
- מגבלת מספר tx tx בתור ותיקוני אתחול Iroha.
- לעטוף Hash במבנה.
- שפר את רמת היומן:
  - הוסף יומני רמת מידע לקונצנזוס.
  - סמן יומני תקשורת ברשת כרמת עקבות.
  - הסר וקטור בלוק מ-WSV מכיוון שהוא שכפול והוא הראה את כל הבלוקצ'יין ביומנים.
  - הגדר את רמת יומן המידע כברירת מחדל.
- הסר הפניות WSV הניתנות לשינוי לצורך אימות.
- תוספת גרסת Heim.
- הוסף ברירת מחדל עמיתים מהימנים לתצורה.
- הגירה של לקוח API ל-http.
- הוסף transfer isi ל-CLI.
- תצורה של הוראות הקשורות לעמיתים Iroha.
- הטמעת שיטות ביצוע ובדיקה חסרות של ISI.
- ניתוח פרמטרים של שאילתת כתובת אתר
- הוסף `HttpResponse::ok()`, `HttpResponse::upgrade_required(..)`
- החלפת דגמי הוראה ושאילתות ישנים בגישת Iroha DSL.
- הוסף תמיכה בחתימות BLS.
- הצג את ארגז השרת http.
- libssl.so.1.0.0 תוקן עם סימלינק.
- מאמת את חתימת החשבון לעסקה.
- שלבי עסקת Refactor.
- שיפורים ראשוניים בדומיינים.
- הטמעת אב טיפוס DSL.
- שפר את מדדי Torii: השבת מדדי כניסה לחשבון, הוסף קביעת יחס הצלחה.
- שפר את צינור כיסוי הבדיקה: מחליף את `tarpaulin` ב-`grcov`, פרסם את דוח כיסוי הבדיקה ל-`codecov.io`.
- תקן נושא RTD.
- משלוח חפצים עבור תת-פרויקטים של iroha.
- הצג את `SignedQueryRequest`.
- תקן באג עם אימות חתימה.
- תמיכה בעסקאות חזרה.
- צמד מפתחות שנוצר בהדפסה כ-json.
- תמיכה בזוג מפתחות `Secp256k1`.
- תמיכה ראשונית באלגוריתמי קריפטו שונים.
- תכונות DEX.
- החלף נתיב תצורה מקודד ב-cli param.
- תיקון זרימת עבודה מאסטר של ספסל.
- בדיקת חיבור אירוע Docker.
- מדריך צג Iroha ו-CLI.
- שיפורים cli אירועים.
- מסנן אירועים.
- חיבורים לאירועים.
- תקן בזרימת העבודה הראשית.
- Rtd עבור iroha2.
- Hash שורש עץ מרקל לעסקאות בלוק.
- פרסום ל- docker hub.
- פונקציונליות CLI עבור Maintenance Connect.
- פונקציונליות CLI עבור Maintenance Connect.
- Eprintln לתיעוד מאקרו.- שיפורים ביומן.
- IR-802 מנוי לחסימת שינויים בסטטוסים.
- שליחת אירועים של עסקאות וחסימות.
- מעביר את הטיפול בהודעות Sumeragi לתוך ההודעה impl.
- מנגנון חיבור כללי.
- חלץ ישויות תחום Iroha עבור לקוח ללא סטד.
- עסקאות TTL.
- מקסימום עסקאות לכל תצורת בלוק.
- אחסן גיבוב של בלוקים לא חוקיים.
- סנכרון בלוקים בקבוצות.
- תצורה של פונקציונליות החיבור.
- התחבר לפונקציונליות Iroha.
- תיקוני אימות חסימה.
- סנכרון בלוקים: דיאגרמות.
- התחבר לפונקציונליות Iroha.
- גשר: הסר לקוחות.
- חסימת סנכרון.
- AddPeer ISI.
- שינוי שם פקודות להוראות.
- נקודת קצה של מדדים פשוטים.
- גשר: קבל גשרים רשומים ונכסים חיצוניים.
- בדיקת חיבור Docker בצנרת.
- אין מספיק קולות מבחן Sumeragi.
- שרשור בלוקים.
- גשר: טיפול ידני בהעברות חיצוניות.
- נקודת קצה תחזוקה פשוטה.
- הגירה ל-serde-json.
- דמינט ISI.
- הוסף הרשאות גשר, AddSignatory ISI ו-CanAddSignatory.
- Sumeragi: עמיתים בתיקוני TODO הקשורים לסט b.
- מאמת את החסימה לפני הכניסה ל-Sumeragi.
- לגשר על נכסים חיצוניים.
- אימות חתימה בהודעות Sumeragi.
- חנות נכסים בינארית.
- החלף את כינוי PublicKey בסוג.
- הכנת ארגזים לפרסום.
- הגיון מינימום הצבעות בתוך NetworkTopology.
- Refactoring של אימות TransactionReceipt.
- שינוי טריגר OnWorldStateViewChange: IrohaQuery במקום Instruction.
- הפרד בנייה מאתחול ב-NetworkTopology.
- הוסף Iroha הוראות מיוחדות הקשורות לאירועי Iroha.
- טיפול בפסק זמן ליצירת בלוק.
- מילון מונחים וכיצד להוסיף מסמכי Iroha.
- החלף את דגם הגשר המקודד במקור בדגם Iroha.
- הצג את מבנה NetworkTopology.
- הוסף ישות הרשאה עם טרנספורמציה מהוראות.
- Sumeragi הודעות במודול ההודעות.
- פונקציונליות בלוק בראשית עבור Kura.
- הוסף קבצי README עבור ארגזים Iroha.
- Bridge ו-RegisterBridge ISI.
- עבודה ראשונית עם Iroha משנה את המאזינים.
- הזרקת בדיקות הרשאה לתוך OOB ISI.
- Docker תיקון מספר עמיתים.
- דוגמה לעמית לעמית.
- טיפול בקבלות עסקה.
- הרשאות Iroha.
- מודול לדקס וארגזים לגשרים.
- תקן מבחן אינטגרציה עם יצירת נכסים עם מספר עמיתים.
- יישום מחדש של מודל הנכסים ב-EC-S-.
- טיפול בזמן קצוב.
- כותרת חסום.
- שיטות הקשורות ל-ISI עבור ישויות תחום.
- ספירת מצב Kura ותצורת עמיתים מהימנים.
- כלל מוך תיעוד.
- הוסף CommittedBlock.
- ניתוק הקורה מ-`sumeragi`.
- בדוק שהעסקאות אינן ריקות לפני יצירת הבלוק.
- יישום מחדש של Iroha הוראות מיוחדות.
- בנצ'מרקים לעסקאות וחוסמות מעברים.
- מחזור החיים והמצבים של העסקאות עובדו מחדש.
- חוסם מחזור חיים ומצבים.
- תקן באג אימות, מחזור לולאה `sumeragi` מסונכרן עם פרמטר התצורה block_build_time_ms.
- עטיפה של אלגוריתם Sumeragi בתוך מודול `sumeragi`.
- מודול לעג עבור ארגז רשת Iroha מיושם באמצעות ערוצים.
- הגירה ל-async-std API.
- תכונת מדומה ברשת.
- ניקוי קוד קשור אסינכרוני.
- מיטוב ביצועים בלולאת עיבוד עסקאות.
- יצירת זוגות מפתח חולצה מההתחלה Iroha.
- אריזה Docker של קובץ הפעלה Iroha.- הצג את התרחיש הבסיסי של Sumeragi.
- לקוח Iroha CLI.
- טיפת iroha לאחר ביצוע קבוצתי בספסל.
- שלב את `sumeragi`.
- שנה את היישום של `sort_peers` ל-rand shuffle עם זרעון של בלוק קודם.
- הסר את מעטפת ההודעה במודול עמיתים.
- עטפו מידע הקשור לרשת בתוך `torii::uri` ו-`iroha_network`.
- הוסף הוראת עמית המיושמת במקום טיפול בקוד קשיח.
- תקשורת עמיתים באמצעות רשימת עמיתים מהימנים.
- עטיפה של טיפול בבקשות רשת בתוך Torii.
- אנקפסולציה של לוגיקה קריפטו בתוך מודול קריפטו.
- סימן חסום עם חותמת זמן ו-hash בלוק קודם כמטען.
- פונקציות קריפטו ממוקמות על גבי המודול ועובדות עם חתם ursa המובלע בחתימה.
- Sumeragi ראשוני.
- אימות הוראות עסקה על שיבוט תצוגת מצב עולמי לפני התחייבות לחנות.
- אימות חתימות על קבלת העסקה.
- תקן באג ב-Request deserialization.
- יישום חתימת Iroha.
- ישות Blockchain הוסרה כדי לנקות את בסיס הקוד.
- שינויים ב-Transactions API: יצירה טובה יותר ועבודה עם בקשות.
- תקן את הבאג שייצור בלוקים עם וקטור ריק של עסקה
- העברה של עסקאות ממתינות.
 - תקן באג עם בתים חסר בחבילת TCP מקודדת u128 Norito.
- תכונה מאקרו עבור מעקב אחר שיטות.
- מודול P2p.
- שימוש ב-iroha_network ב-torii ובלקוח.
- הוסף מידע ISI חדש.
- כינוי מסוג ספציפי למצב רשת.
- Box<dyn Error> הוחלף במחרוזת.
- האזנה ממלכתית ברשת.
- היגיון אימות ראשוני לעסקאות.
- ארגז רשת Iroha.
- הפקת מאקרו עבור תכונות Io, IntoContract ו-IntoQuery.
- הטמעת שאילתות עבור Iroha-client.
- הפיכת פקודות לחוזי ISI.
- הוסף עיצוב מוצע עבור multisig מותנה.
- הגירה לשטחי עבודה של Cargo.
- הגירת מודולים.
- תצורה חיצונית באמצעות משתני סביבה.
- טיפול בבקשות קבל ושם עבור Torii.
- תיקון Github ci.
- Cargo-make מנקה בלוקים לאחר בדיקה.
- הצג מודול `test_helper_fns` עם פונקציה לניקוי ספרייה עם בלוקים.
- יישום אימות באמצעות עץ merkle.
- הסר נגזרת שאינה בשימוש.
- הפצת אסינכרון/המתנה ותקן את `wsv::put` ללא ציפייה.
- השתמש בהצטרפות מארג `futures`.
- הטמעת ביצוע מאגר מקביל: כתיבה לדיסק ועדכון WSV מתרחשים במקביל.
- השתמש בהפניות במקום בעלות לצורך (דה) סדרה.
- פליטת קוד מקבצים.
- השתמש ב-ursa::blake2.
- חוק לגבי mod.rs במדריך התורמים.
- Hash 32 בתים.
- Hash של בלייק2.
- דיסק מקבל הפניות לחסימה.
- Refactoring של מודול פקודות ועץ Merkle Initial.
- מבנה מודולים משוחזרים.
- עיצוב נכון.
- הוסף הערות למסמך ל-read_all.
- יישם את `read_all`, ארגן מחדש בדיקות אחסון והפוך בדיקות עם פונקציות אסינכרון לבדיקות אסינכרון.
- הסר לכידה ניתנת לשינוי מיותר.
- בדוק את הבעיה, תקן את clippy.
- הסר מקף.
- הוסף בדיקת פורמט.
- הוסף אסימון.
- צור rust.yml עבור פעולות github.
- הצג אב טיפוס לאחסון דיסק.
- בדיקת ופונקציונליות של העברת נכסים.
- הוסף אתחול ברירת המחדל למבנים.
- שנה את השם של מבנה MSTCache.
- הוסף שאלה שנשכחה.
- מתאר ראשוני של קוד iroha2.
- API ראשוני של Kura.
- הוסף כמה קבצים בסיסיים ושחרר גם את הטיוטה הראשונה של הספר הלבן המתאר את החזון עבור iroha v2.
- סניף iroha v2 בסיסי.

## [1.5.0] - 2022-04-08

### שינויים ב-CI/CD
- הסר את Jenkinsfile ואת JenkinsCI.

### נוסף- הוסף יישום אחסון RocksDB עבור Burrow.
- הצג אופטימיזציה של תנועה עם Bloom-filter
- עדכן את רשת המודולים של `MST` כך שתהיה ממוקמת במודול `OS` ב-`batches_cache`.
- הצע אופטימיזציה לתנועה.

### תיעוד

- תקן את המבנה. הוסף הבדלי DB, שיטות הגירה, נקודת קצה בריאותית, מידע על כלי iroha-swarm.

### אחר

- תיקון דרישה עבור בניית מסמך.
- חתוך את תיעוד השחרור כדי להדגיש את פריט המעקב הקריטי שנותר.
- תקן 'בדוק אם תמונת docker קיימת' /בנה את כל skip_testing.
- /build all skip_testing.
- /build skip_testing; ועוד מסמכים.
- הוסף `.github/_README.md`.
- הסר את `.packer`.
- הסר שינויים בפרמטר הבדיקה.
- השתמש בפרמטר חדש כדי לדלג על שלב הבדיקה.
- הוסף לזרימת העבודה.
- הסר את שליחת המאגר.
- הוסף שיגור מאגר.
- הוסף פרמטר לבודקים.
- הסר פסק זמן של `proposal_delay`.

## [1.4.0] - 31-01-2022

### נוסף

- הוסף מצב צומת סנכרון
- מוסיף מדדים עבור RocksDB
- הוסף ממשקי בדיקת בריאות באמצעות http ומדדים.

### תיקונים

- תקן משפחות עמודות ב-Iroha v1.4-rc.2
- הוסף מסנן פריחה של 10 סיביות ב-Iroha v1.4-rc.1

### תיעוד

- הוסף zip ו-pkg-config לרשימת הגדרות הבנייה.
- עדכן את ה-readme: תקן קישורים שבורים כדי לבנות סטטוס, מדריך לבנות וכן הלאה.
- תקן מדדי Config ו-Docker.

### אחר

- עדכון תג GHA docker.
- תקן שגיאות קומפילציה של Iroha 1 בעת קומפילציה עם g++11.
- החלף את `max_rounds_delay` ב-`proposal_creation_timeout`.
- עדכן קובץ תצורה לדוגמה כדי להסיר פרמטרים ישנים של חיבור DB.