<!-- Hebrew translation of docs/source/data_model_and_isi_spec.md -->

---
lang: he
direction: rtl
source: docs/source/data_model_and_isi_spec.md
status: complete
translator: machine-google-reviewed
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
---

# Iroha v2 מודל נתונים ו-ISI — מפרט נגזר מיישום

מפרט זה עבר הנדסה הפוכה מהיישום הנוכחי של `iroha_data_model` ו-`iroha_core` כדי לסייע בסקירת עיצוב. נתיבים ב-backticks מצביעים על הקוד הסמכותי.

## היקף
- מגדיר ישויות קנוניות (דומיינים, חשבונות, נכסים, NFTs, תפקידים, הרשאות, עמיתים, טריגרים) והמזהים שלהם.
- מתאר הוראות לשינוי מצב (ISI): סוגים, פרמטרים, תנאים מוקדמים, מעברי מצב, אירועים שנפלטו ותנאי שגיאה.
- מסכם ניהול פרמטרים, עסקאות והסדרת הוראות.

דטרמיניזם: כל סמנטיקה של הוראות הן מעברי מצב טהורים ללא התנהגות תלוית חומרה. סדרה משתמשת ב-Norito; VM bytecode משתמש ב-IVM ומאומת בצד המארח לפני ביצוע בשרשרת.

---

## ישויות ומזהים
לזהות יש צורות מחרוזות יציבות עם `Display`/`FromStr` הלוך ושוב. כללי השמות אוסרים על רווח לבן ועל תווי `@ # $` השמורים.- `Name` - מזהה טקסטואלי מאומת. כללים: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. דומיין: `{ id, logo, metadata, owned_by }`. בונים: `NewDomain`. קוד: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — כתובות קנוניות מופקות באמצעות `AccountAddress` (I105 / hex) ו-Torii מנרמל כניסות דרך `AccountAddress::parse_encoded`. I105 הוא פורמט החשבון המועדף; טופס I105 מיועד ל-Sora בלבד UX. המחרוזת המוכרת `alias` (טופס מדור קודם) נשמרת ככינוי ניתוב בלבד. חשבון: `{ id, metadata }`. קוד: `crates/iroha_data_model/src/account.rs`.- מדיניות כניסת חשבון - דומיינים שולטים ביצירת חשבון מרומז על ידי אחסון Norito-JSON `AccountAdmissionPolicy` תחת מפתח מטא נתונים `iroha:account_admission_policy`. כאשר המפתח נעדר, הפרמטר המותאם אישית ברמת השרשרת `iroha:default_account_admission_policy` מספק את ברירת המחדל; כאשר זה גם נעדר, ברירת המחדל הקשה היא `ImplicitReceive` (מהדורה ראשונה). תגי המדיניות `mode` (`ExplicitOnly` או `ImplicitReceive`) בתוספת אופציונליים לכל עסקה (ברירת מחדל `16`) ומכסי יצירה לכל בלוק, חשבון `mode` אופציונלי (I0burn או 8NI40X) הגדרת נכס, ו-`default_role_on_create` אופציונלי (ניתן לאחר `AccountCreated`, נדחה עם `DefaultRoleError` אם חסר). בראשית אין אפשרות להצטרף; מדיניות מושבתת/לא חוקית דוחה הוראות בסגנון קבלה עבור חשבונות לא ידועים עם `InstructionExecutionError::AccountAdmission`. חשבונות מרומזים חותמים מטא נתונים `iroha:created_via="implicit"` לפני `AccountCreated`; תפקידי ברירת המחדל מוציאים מעקב `AccountRoleGranted`, וכללי הבסיס של הבעלים המוציאים לפועל מאפשרים לחשבון החדש להוציא את הנכסים/NFT שלו ללא תפקידים נוספים. קוד: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` - `aid:<32-lower-hex-no-dash>` קנוני (UUID-v4 בתים). הגדרה: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. ספרות `alias` חייבות להיות `<name>#<domain>@<dataspace>` או `<name>#<dataspace>`, כאשר `<name>` שווה לשם הגדרת הנכס. קוד: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: מילולית מקודדת קנונית `norito:<hex>` (טפסים טקסטואליים מדור קודם אינם נתמכים במהדורה הראשונה).- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. קוד: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. תפקיד: `{ id, permissions: BTreeSet<Permission> }` עם Builder `NewRole { inner: Role, grant_to }`. קוד: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. קוד: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` - זהות עמיתים (מפתח ציבורי) וכתובת. קוד: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. טריגר: `{ id, action }`. פעולה: `{ executable, repeats, authority, filter, metadata }`. קוד: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` עם הוספה/הסרה מסומנת. קוד: `crates/iroha_data_model/src/metadata.rs`.
- דפוס מנוי (שכבת אפליקציה): התוכניות הן ערכי `AssetDefinition` עם מטא נתונים `subscription_plan`; המנויים הם רשומות `Nft` עם מטא נתונים של `subscription`; החיוב מבוצע על ידי מפעילי זמן המתייחסים ל-NFT של מנויים. ראה `docs/source/subscriptions_api.md` ו-`crates/iroha_data_model/src/subscription.rs`.
- **פרימיטיבים קריפטוגרפיים** (תכונה `sm`):
  - `Sm2PublicKey` / `Sm2Signature` משקפים את נקודת SEC1 הקנונית + קידוד `r∥s` ברוחב קבוע עבור SM2. הבנאים אוכפים חברות בעקומה וסמנטיקה מבדלת של מזהה (`DEFAULT_DISTID`), בעוד שהאימות דוחה סקלרים עם מבנה שגוי או בטווח גבוה. קוד: `crates/iroha_crypto/src/sm.rs` ו-`crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` חושף את תקציר ה-GM/T 0004 כסוג חדש מסוג `[u8; 32]` הניתן להמשכה Norito בשימוש בכל מקום שבו מופיעים גיבובים במניפסטים או בטלמטריה. קוד: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` מייצג מפתחות SM4 של 128 סיביות ומשותף בין מערכות מערכות מארח ותקני מודל נתונים. קוד: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  סוגים אלה יושבים לצד הפרימיטיבים הקיימים של Ed25519/BLS/ML-DSA וזמינים לצרכנים של מודל נתונים (Torii, SDKs, כלי בראשית) ברגע שתכונת `sm` מופעלת.
- מאגרי קשרים הנגזרים ממרחב נתונים (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, רישום חירום של ממסר נתיב) והרשאות יעדי מרחב נתונים (I101rol1100 מותרות באחסון בחשבון) ב-`State::set_nexus(...)` כאשר מרחבי נתונים נעלמים מה-`dataspace_catalog` הפעיל, מונע הפניות מיושנות למרחבי נתונים לאחר עדכוני קטלוג זמן ריצה. מטמון DA/ממסר בהיקף נתיב (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) נגזמים גם כאשר נתיב מופסק או מוקצה מחדש למרחב נתונים אחר ולכן לא ניתן להעביר נתיב בין מצב נתונים מקומי. ISIs של מדריך החלל (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) מאמתים גם את `dataspace` מול הקטלוג הפעיל ודוחים מזהים לא ידועים עם `InvalidParameter`.

תכונות חשובות: `Identifiable`, `Registered`/`Registrable` (תבנית בונה), `HasMetadata`, `IntoKeyValue`. קוד: `crates/iroha_data_model/src/lib.rs`.

אירועים: לכל ישות יש אירועים הנפלטים על מוטציות (יצירה/מחיקה/שינוי בעלים/מטא נתונים וכו'). קוד: `crates/iroha_data_model/src/events/`.

---## פרמטרים (תצורת שרשרת)
- משפחות: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, בתוספת `custom: BTreeMap`.
- תקנות בודדות ל-Diffs: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. אגרגטור: `Parameters`. קוד: `crates/iroha_data_model/src/parameter/system.rs`.

הגדרת פרמטרים (ISI): `SetParameter(Parameter)` מעדכן את השדה המתאים ופולט `ConfigurationEvent::Changed`. קוד: `crates/iroha_data_model/src/isi/transparent.rs`, מבצע ב-`crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## הסדרת הוראות ורישום
- תכונת ליבה: `Instruction: Send + Sync + 'static` עם `dyn_encode()`, `as_any()`, `id()` יציב (ברירת המחדל של שם סוג הבטון).
- `InstructionBox`: עטיפה `Box<dyn Instruction>`. Clone/Eq/Ord פועלים על `(type_id, encoded_bytes)` כך שהשוויון הוא לפי ערך.
- Norito serde עבור `InstructionBox` מסודרת כ-`(String wire_id, Vec<u8> payload)` (נופל בחזרה ל-`type_name` אם אין מזהה חוט). דה-סריאליזציה משתמשת במזהי מיפוי גלובלי של `InstructionRegistry` לבנאים. רישום ברירת המחדל כולל את כל ה-ISI המובנה. קוד: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: סוגים, סמנטיקה, שגיאות
הביצוע מיושם באמצעות `Execute for <Instruction>` ב-`iroha_core::smartcontracts::isi`. להלן מפרט את ההשפעות הציבוריות, התנאים המוקדמים, האירועים הנפלטים והטעויות.

### הרשמה / בטל רישום
סוגים: `Register<T: Registered>` ו-`Unregister<T: Identifiable>`, עם סוגי סכום `RegisterBox`/`UnregisterBox` המכסים מטרות בטון.- רשום עמית: מוסיף לקבוצת עמיתים בעולם.
  - תנאים מוקדמים: אסור להתקיים כבר.
  - אירועים: `PeerEvent::Added`.
  - שגיאות: `Repetition(Register, PeerId)` אם שכפול; `FindError` על חיפושים. קוד: `core/.../isi/world.rs`.

- רישום דומיין: בונה מ-`NewDomain` עם `owned_by = authority`. אסור: דומיין `genesis`.
  - תנאים מוקדמים: אי קיום תחום; לא `genesis`.
  - אירועים: `DomainEvent::Created`.
  - שגיאות: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. קוד: `core/.../isi/world.rs`.

- רישום חשבון: בונה מ-`NewAccount`, אסור בדומיין `genesis`; לא ניתן לרשום חשבון `genesis`.
  - תנאים מוקדמים: תחום חייב להתקיים; אי קיום חשבון; לא בתחום בראשית.
  - אירועים: `DomainEvent::Account(AccountEvent::Created)`.
  - שגיאות: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. קוד: `core/.../isi/domain.rs`.

- רשום AssetDefinition: בונה מ-Builder; ערכות `owned_by = authority`.
  - תנאים מוקדמים: הגדרה אי קיום; קיים דומיין; `name` נדרש, חייב להיות לא ריק לאחר חיתוך, ואסור להכיל `#`/`@`.
  - אירועים: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - שגיאות: `Repetition(Register, AssetDefinitionId)`. קוד: `core/.../isi/domain.rs`.

- רישום NFT: בונה מבונה; ערכות `owned_by = authority`.
  - תנאים מוקדמים: אי-קיום NFT; דומיין קיים.
  - אירועים: `DomainEvent::Nft(NftEvent::Created)`.
  - שגיאות: `Repetition(Register, NftId)`. קוד: `core/.../isi/nft.rs`.- תפקיד רישום: בונה מ-`NewRole { inner, grant_to }` (הבעלים הראשון מתועד באמצעות מיפוי תפקידים בחשבון), מאחסן את `inner: Role`.
  - תנאים מוקדמים: אי קיום תפקיד.
  - אירועים: `RoleEvent::Created`.
  - שגיאות: `Repetition(Register, RoleId)`. קוד: `core/.../isi/world.rs`.

- Register Trigger: מאחסן את הטריגר בטריגר המתאים שנקבע לפי סוג המסנן.
  - תנאים מוקדמים: אם המסנן אינו ניתן להטבעה, `action.repeats` חייב להיות `Exactly(1)` (אחרת `MathError::Overflow`). אסור לשכפל תעודות זהות.
  - אירועים: `TriggerEvent::Created(TriggerId)`.
  - שגיאות: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` על כשלי המרה/אימות. קוד: `core/.../isi/triggers/mod.rs`.- בטל רישום של עמית/דומיין/חשבון/נכס הגדרה/NFT/תפקיד/טריגר: מסיר את היעד; פולט אירועי מחיקה. הסרות מדורגות נוספות:- בטל רישום דומיין: מסיר את ישות הדומיין בתוספת מצב הבורר/הסמכה שלה; מוחק הגדרות נכסים בדומיין (ומצב צדדי סודי `zk_assets` המבוסס על הגדרות אלו), נכסים של הגדרות אלו (ומטא-נתונים לכל נכס), NFTs בדומיין ותחזיות תווית חשבון/כינוי בהיקף של תחום. זה גם מבטל את הקישור של חשבונות שרדו מהדומיין שהוסר וגזם ערכי הרשאות בהיקף חשבון/תפקיד המתייחסים לדומיין שהוסר או למשאבים שנמחקו איתו (הרשאות דומיין, הרשאות הגדרת נכס/נכס עבור הגדרות שהוסרו והרשאות NFT עבור מזהי NFT שהוסרו). הסרת הדומיין אינה מוחקת את ה-`AccountId` העולמי, מצב ה-TX-sequence/UAID שלו, בעלות על נכס זר או NFT, סמכות מפעילה או אזכורים חיצוניים לביקורת/תצורה חיצונית המצביעים על החשבון שנשאר בחיים. מעקות שמירה: דוחה כאשר כל הגדרת נכס בדומיין עדיין מוזכרת על ידי הסכם ריפו, פנקס חשבונות, תגמול/תביעה במסלול ציבורי, קצבה/העברה לא מקוונת, ברירת מחדל של ריפו של התנחלות (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), רשות ממשלתית/התאמת-התנחלות הפניות להגדרת נכסים, הפניות להגדרת תגמול/חותך/מחלוקת בהגדרת נכסים של נכסים, או הפניות להגדרת עמלת Nexus/הגדרת נכסים (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). אירועים: `DomainEvent::Deleted`, בתוספת מחיקה לכל פריטעל אירועים עבור משאבים בהיקף של דומיין שהוסרו. שגיאות: `FindError::Domain` אם חסר; `InvariantViolation` על התנגשויות התייחסות להגדרה של נכס שנשאר. קוד: `core/.../isi/world.rs`.- בטל רישום חשבון: מסיר את ההרשאות, התפקידים, מונה ה-tx-sequence של החשבון, מיפוי תוויות החשבון וקשרי UAID; מוחק נכסים שבבעלות החשבון (ומטא נתונים לכל נכס); מוחק NFTs בבעלות החשבון; מסיר טריגרים שסמכותם היא החשבון הזה; גזם ערכי הרשאות בחשבון/תפקידים המתייחסים לחשבון שהוסר, הרשאות יעדי NFT בהיקף חשבון/תפקיד עבור מזהי NFT שבבעלותם שהוסרו, והרשאות יעדי טריגר בהיקף חשבון/תפקיד עבור טריגרים שהוסרו. מעקות שמירה: דוחה אם החשבון עדיין מחזיק בדומיין, הגדרת נכס, כריכת ספק SoraFS, רשומת אזרחות פעילה, מצב הימור/תגמול בנתיב ציבורי (כולל מפתחות תביעת פרס כאשר החשבון מופיע כתובע או בעל נכס פרס), מדינת אורקל פעילה (כולל ספק פיד או twitter, ספק פיד או twitter הפניות לחשבון תגמול/נתח שמוגדר ב-oracle-economics), הפניות פעילות של Nexus עמלות/חשבון הימור (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; מנותח כ-Nexus פעילים ב-`nexus.fees.fee_sink_account_id`, ב-`nexus.staking.stake_escrow_account_id`, מדינה, מצב פנקס חשבונות פעיל, קצבה/העברה לא מקוונת פעילה או מצב ביטול פסק דין לא מקוון, הפניות פעילות של תצורת חשבון נאמנות לא מקוון עבור הגדרות נכסים פעילים (`settlement.offline.escrow_accounts`), מצב ממשל פעיל (הצעה/אישור שלבals/locks/slashes/council/parliment lists, תמונות מצב של הצעות לפרלמנט, רשומות מגישי הצעות שדרוג בזמן ריצה, הפניות לחשבון נאמנות/slash-receiver/viral-pool בתצורת ממשל, ממשל SoraFS הפניות למגישי טלמטריה דרך /I102NI100X0 / I102NI100X0 `gov.sorafs_telemetry.per_provider_submitters`, או הפניות לבעלים של SoraFS עם תצורת ממשל באמצעות `gov.sorafs_provider_owners`), תוכן מוגדר פרסם הפניות לחשבון רשימת היתרים (`content.publish_allow_accounts`), שולח נאמנות חברתי פעיל מדינה, יוצר מצב חירום פעיל מצב חירום, יוצר מצב חירום פעיל חבילה מצב עקיפה של validator, או SoraFS רשומות מנפיק/קלסר סיכות (מניפסטי סיכה, כינויים של מניפסט, סדרי שכפול) פעילים. אירועים: `AccountEvent::Deleted`, בתוספת `NftEvent::Deleted` לכל NFT שהוסר. שגיאות: `FindError::Account` אם חסר; `InvariantViolation` על יתומי בעלות. קוד: `core/.../isi/domain.rs`.- Unregister AssetDefinition: מוחק את כל הנכסים של הגדרה זו ואת המטא נתונים שלהם לכל נכס, ומסיר מצב צדדי סודי `zk_assets` שנקבע על ידי הגדרה זו; חותך גם את הערך התואם `settlement.offline.escrow_accounts` ואת ערכי ההרשאה בהיקף חשבון/תפקיד המתייחסים להגדרת הנכס שהוסר או למופעי הנכס שלו. מעקות שמירה: דוחה כאשר ההגדרה עדיין מוזכרת על ידי הסכם ריפו, ספר התנחלויות, תגמול/תביעה במסלול ציבורי, קצבה לא מקוונת/מצב העברה, ברירות מחדל של ריפו של התנחלויות (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), זכות-הצבעה/אזרח-הצבעה בתצורת ממשל הפניות, אזכורים של תגמול/חיתוך/מחלוקת בהגדרת נכסים ב-oracle-economics, או הפניות ל-Nexus עמלה/הגדרת נכס הימור (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). אירועים: `AssetDefinitionEvent::Deleted` ו-`AssetEvent::Deleted` לכל נכס. שגיאות: `FindError::AssetDefinition`, `InvariantViolation` על התנגשויות התייחסות. קוד: `core/.../isi/domain.rs`.
  - בטל רישום של NFT: מסיר NFT וגזום ערכי הרשאות בהיקף חשבון/תפקיד המתייחסים ל-NFT שהוסר. אירועים: `NftEvent::Deleted`. שגיאות: `FindError::Nft`. קוד: `core/.../isi/nft.rs`.
  - בטל רישום תפקיד: מבטל את התפקיד מכל החשבונות תחילה; לאחר מכן מסיר את התפקיד. אירועים: `RoleEvent::Deleted`. שגיאות: `FindError::Role`. קוד: `core/.../isi/world.rs`.- בטל רישום טריגר: מסיר טריגר אם קיים וגזום ערכי הרשאות בהיקף חשבון/תפקיד המתייחסים לטריגר שהוסר; ביטול רישום כפול מניב `Repetition(Unregister, TriggerId)`. אירועים: `TriggerEvent::Deleted`. קוד: `core/.../isi/triggers/mod.rs`.

### מנטה/צריבה
סוגים: `Mint<O, D: Identifiable>` ו-`Burn<O, D: Identifiable>`, באריזה כ-`MintBox`/`BurnBox`.

- נכס (נומרי) מנטה/צריבה: מתאים יתרות והגדרות `total_quantity`.
  - תנאים מוקדמים: ערך `Numeric` חייב לעמוד ב-`AssetDefinition.spec()`; הטבעה מותרת על ידי `mintable`:
    - `Infinitely`: מותר תמיד.
    - `Once`: מותר פעם אחת בדיוק; המנטה הראשונה הופכת את `mintable` ל-`Not` ופולטת `AssetDefinitionEvent::MintabilityChanged`, בתוספת `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` מפורטת לביקורת.
    - `Limited(n)`: מאפשר `n` פעולות מנטה נוספות. כל מנטה מוצלחת מורידה את הדלפק; כאשר הוא מגיע לאפס, ההגדרה מתהפכת ל-`Not` ופולטת את אותם אירועי `MintabilityChanged` כמו לעיל.
    - `Not`: שגיאה `MintabilityError::MintUnmintable`.
  - שינויים במדינה: יוצר נכס אם חסר במטבעה; מסיר את הזנת הנכס אם היתרה הופכת לאפס בצריבה.
  - אירועים: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (כאשר `Once` או `Limited(n)` ממצה את הקצבה).
  - שגיאות: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. קוד: `core/.../isi/asset.rs`.- טריגר חזרות מנטה/צריבה: משנה את ספירת `action.repeats` עבור טריגר.
  - תנאים מוקדמים: על מנטה, המסנן חייב להיות ניתן לטבעה; אריתמטיקה לא חייבת לעלות על גדותיה/להתפרק.
  - אירועים: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - שגיאות: `MathError::Overflow` על מנטה לא חוקית; `FindError::Trigger` אם חסר. קוד: `core/.../isi/triggers/mod.rs`.

### העברה
סוגים: `Transfer<S: Identifiable, O, D: Identifiable>`, באריזה כ-`TransferBox`.

- נכס (מספרי): הורידו ממקור `AssetId`, הוסף ליעד `AssetId` (אותה הגדרה, חשבון שונה). מחק נכס מקור מאופס.
  - תנאים מוקדמים: קיים נכס מקור; הערך עונה על `spec`.
  - אירועים: `AssetEvent::Removed` (מקור), `AssetEvent::Added` (יעד).
  - שגיאות: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. קוד: `core/.../isi/asset.rs`.

- בעלות על דומיין: משנה את `Domain.owned_by` לחשבון היעד.
  - תנאים מוקדמים: שני החשבונות קיימים; דומיין קיים.
  - אירועים: `DomainEvent::OwnerChanged`.
  - שגיאות: `FindError::Account/Domain`. קוד: `core/.../isi/domain.rs`.

- בעלות AssetDefinition: משנה את `AssetDefinition.owned_by` לחשבון היעד.
  - תנאים מוקדמים: שני החשבונות קיימים; הגדרה קיימת; המקור חייב כרגע להחזיק בו; הסמכות חייבת להיות חשבון מקור, בעל תחום מקור או בעל תחום הגדרת נכס.
  - אירועים: `AssetDefinitionEvent::OwnerChanged`.
  - שגיאות: `FindError::Account/AssetDefinition`. קוד: `core/.../isi/account.rs`.- בעלות על NFT: משנה את `Nft.owned_by` לחשבון היעד.
  - תנאים מוקדמים: שני החשבונות קיימים; NFT קיים; המקור חייב כרגע להחזיק בו; הסמכות חייבת להיות חשבון מקור, בעל דומיין מקור, בעל דומיין NFT, או להחזיק `CanTransferNft` עבור אותו NFT.
  - אירועים: `NftEvent::OwnerChanged`.
  - שגיאות: `FindError::Account/Nft`, `InvariantViolation` אם המקור אינו הבעלים של ה-NFT. קוד: `core/.../isi/nft.rs`.

### מטא נתונים: הגדר/הסר ערך מפתח
סוגים: `SetKeyValue<T>` ו-`RemoveKeyValue<T>` עם `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. רישומים בקופסה מסופקים.

- סט: מכניס או מחליף את `Metadata[key] = Json(value)`.
- הסר: מסיר את המפתח; שגיאה אם ​​חסר.
- אירועים: `<Target>Event::MetadataInserted` / `MetadataRemoved` עם הערכים הישנים/חדשים.
- שגיאות: `FindError::<Target>` אם היעד אינו קיים; `FindError::MetadataKey` על מפתח חסר להסרה. קוד: `crates/iroha_data_model/src/isi/transparent.rs` ו-executor impls לכל יעד.

### הרשאות ותפקידים: הענק/בטל
סוגים: `Grant<O, D>` ו-`Revoke<O, D>`, עם רשומות אריזות עבור `Permission`/`Role` עד/מ `Account`, ו-`Role` עד `Role`.- הענק הרשאה לחשבון: מוסיף את `Permission` אלא אם כבר אינהרנטית. אירועים: `AccountEvent::PermissionAdded`. שגיאות: `Repetition(Grant, Permission)` אם שכפול. קוד: `core/.../isi/account.rs`.
- בטל הרשאה מהחשבון: מסיר אם קיים. אירועים: `AccountEvent::PermissionRemoved`. שגיאות: `FindError::Permission` אם נעדר. קוד: `core/.../isi/account.rs`.
- הענק תפקיד לחשבון: מוסיף מיפוי `(account, role)` אם נעדר. אירועים: `AccountEvent::RoleGranted`. שגיאות: `Repetition(Grant, RoleId)`. קוד: `core/.../isi/account.rs`.
- בטל תפקיד מחשבון: מסיר מיפוי אם קיים. אירועים: `AccountEvent::RoleRevoked`. שגיאות: `FindError::Role` אם נעדר. קוד: `core/.../isi/account.rs`.
- הענק הרשאה לתפקיד: בונה מחדש את התפקיד עם הרשאה נוספת. אירועים: `RoleEvent::PermissionAdded`. שגיאות: `Repetition(Grant, Permission)`. קוד: `core/.../isi/world.rs`.
- בטל הרשאה מתפקיד: בונה מחדש את התפקיד ללא הרשאה זו. אירועים: `RoleEvent::PermissionRemoved`. שגיאות: `FindError::Permission` אם נעדר. קוד: `core/.../isi/world.rs`.### טריגרים: בצע
סוג: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- התנהגות: מעמיד בתור `ExecuteTriggerEvent { trigger_id, authority, args }` עבור תת-מערכת ההדק. הפעלה ידנית מותרת רק עבור מפעילי קריאת עזר (מסנן `ExecuteTrigger`); המסנן חייב להתאים והמתקשר חייב להיות הרשות לפעולת ההפעלה או להחזיק `CanExecuteTrigger` עבור אותה סמכות. כאשר מבצע שסופק על ידי משתמש פעיל, ביצוע הטריגר מאומת על ידי מבצע זמן הריצה וצורך את תקציב הדלק של מבצע העסקה (בסיס `executor.fuel` בתוספת מטא נתונים אופציונליים `additional_fuel`).
- שגיאות: `FindError::Trigger` אם לא רשום; `InvariantViolation` אם התקשרו ללא רשות. קוד: `core/.../isi/triggers/mod.rs` (ובדיקות ב-`core/.../smartcontracts/isi/mod.rs`).

### שדרוג והתחבר
- `Upgrade { executor }`: מעביר את המבצע באמצעות קוד בתים `Executor` שסופק, מעדכן את המבצע ואת מודל הנתונים שלו, פולט `ExecutorEvent::Upgraded`. שגיאות: עטוף כ-`InvalidParameterError::SmartContract` על כשל הגירה. קוד: `core/.../isi/world.rs`.
- `Log { level, msg }`: פולט יומן צומת עם הרמה הנתונה; ללא שינויי מדינה. קוד: `core/.../isi/world.rs`.

### מודל שגיאה
מעטפה נפוצה: `InstructionExecutionError` עם גרסאות לשגיאות הערכה, כשלים בשאילתה, המרות, ישות לא נמצאה, חזרה, יכולת טביעה, מתמטיקה, פרמטר לא חוקי והפרה בלתי משתנה. ספירות ועוזרים נמצאים ב-`crates/iroha_data_model/src/isi/mod.rs` תחת `pub mod error`.

---## עסקאות ואפשרויות הפעלה
- `Executable`: או `Instructions(ConstVec<InstructionBox>)` או `Ivm(IvmBytecode)`; bytecode מסתמן כ-base64. קוד: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: בונה, מסמן ואורז קובץ הפעלה עם מטא נתונים, `chain_id`, `authority`, `creation_time_ms`, אופציונלי `chain_id`, `authority`, `creation_time_ms`, `chain_id`, אופציונלי `nonce`. קוד: `crates/iroha_data_model/src/transaction/`.
- בזמן ריצה, `iroha_core` מבצע אצווה `InstructionBox` באמצעות `Execute for InstructionBox`, מורידה להוראת `*Box` המתאימה או קונקרטית. קוד: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- תקציב אימות מבצעים בזמן ריצה (מבצע שסופק על ידי המשתמש): בסיס `executor.fuel` מפרמטרים בתוספת מטא נתונים אופציונליים של עסקאות `additional_fuel` (`u64`), משותף בין אימותי הוראות/טריגרים בתוך העסקה.

---## אינוריאנטים והערות (מבדיקות ושומרים)
- הגנות בראשית: אין אפשרות לרשום את הדומיין `genesis` או חשבונות בדומיין `genesis`; לא ניתן לרשום חשבון `genesis`. קוד/בדיקות: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- נכסים מספריים חייבים לעמוד ב-`NumericSpec` שלהם על מטבעות/העברה/צריבה; אי התאמה של המפרט מניב `TypeError::AssetNumericSpec`.
- יכולת טביעה: `Once` מאפשר מנטה בודדת ולאחר מכן מתהפך ל-`Not`; `Limited(n)` מאפשר בדיוק `n` מנטה לפני היפוך ל-`Not`. ניסיונות לאסור טביעה על `Infinitely` גורמים ל-`MintabilityError::ForbidMintOnMintable`, והגדרת `Limited(0)` מניבה `MintabilityError::InvalidMintabilityTokens`.
- פעולות המטא-נתונים הן המפתח-מדויק; הסרת מפתח לא קיים היא שגיאה.
- מסנני טריגר יכולים להיות בלתי ניתנים להטבעה; אז `Register<Trigger>` מאפשר רק חזרות של `Exactly(1)`.
- הפעל מפתח מטא נתונים `__enabled` (bool) שערי ביצוע; ברירות מחדל חסרות לאפשרות, וטריגרים מושבתים מדלגים בנתיבי נתונים/זמן/לפי שיחה.
- דטרמיניזם: כל האריתמטיקה משתמשת בפעולות מסומנות; under/overflow מחזיר שגיאות מתמטיות מוקלדות; אפס יתרות מורידות כניסות נכסים (ללא מצב נסתר).

---## דוגמאות מעשיות
- טביעה והעברה:
  - `Mint::asset_numeric(10, asset_id)` ← מוסיף 10 אם מותר לפי מפרט/יכולת נטיעה; אירועים: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → מהלכים 5; אירועים להסרה/הוספה.
- עדכוני מטא נתונים:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → העלאה; הסרה באמצעות `RemoveKeyValue::account(...)`.
- ניהול תפקיד/הרשאות:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)`, ועמיתיהם `Revoke`.
- מחזור חיי טריגר:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` עם בדיקת יכולת הטביעה המשתמעת על ידי מסנן; `ExecuteTrigger::new(id).with_args(&args)` חייב להתאים לסמכות שהוגדרה.
  - ניתן להשבית טריגרים על ידי הגדרת מפתח מטא נתונים `__enabled` ל-`false` (חסרות ברירת מחדל למופעלת); החלף דרך `SetKeyValue::trigger` או IVM `set_trigger_enabled`.
  - אחסון טריגרים מתוקן בעת ​​טעינה: מזהים כפולים, מזהים לא תואמים וטריגרים המתייחסים לקוד בתים חסר נשמטים; ספירת הפניות של קוד בתים מחושבת מחדש.
  - אם קוד הבתים IVM של טריגר חסר בזמן הביצוע, הטריגר מוסר והביצוע יטופל כאי-אופ עם תוצאה של כשל.
  - טריגרים מדולדלים מוסרים מיד; אם נתקלים בכניסה מדולדלת במהלך הביצוע היא נגזמת ומטופלת כחסרה.
- עדכון פרמטר:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` מעדכן ופולט `ConfigurationEvent::Changed`.CLI / Torii `aid` + דוגמאות כינוי:
- הרשמה בסיוע קנוני + שם מפורש + כינוי ארוך:
  - `iroha ledger asset definition register --id aid:2f17c72466f84a4bb8a8e24884fdcd2f --name pkr --alias pkr#ubl@sbp`
- הרשמה בסיוע קנוני + שם מפורש + כינוי קצר:
  - `iroha ledger asset definition register --id aid:550e8400e29b41d4a7164466554400dd --name pkr --alias pkr#sbp`
- נטבע בכינוי + רכיבי חשבון:
  - `iroha ledger asset mint --definition-alias pkr#ubl@sbp --account <i105> --quantity 500`
- פתרון כינוי לסיוע קנוני:
  - `POST /v1/assets/aliases/resolve` עם JSON `{ "alias": "pkr#ubl@sbp" }`

הערת הגירה:
- מזהי `name#domain` הגדרת נכסים טקסטואליים אינם נתמכים בכוונה במהדורה הראשונה.
- מזהי נכסים בגבולות הטבעה/שריפה/העברה נשארים קנוניים `norito:<hex>`; השתמש ב-`iroha tools encode asset-id` עם `--definition aid:...` או `--alias ...` בתוספת `--account`.

---

## יכולת מעקב (מקורות נבחרים)
 - ליבת דגם נתונים: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - הגדרות ורישום ISI: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ביצוע ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - אירועים: `crates/iroha_data_model/src/events/**`.
 - עסקאות: `crates/iroha_data_model/src/transaction/**`.

אם ברצונך להרחיב את המפרט הזה ל-API/טבלת התנהגות מעובדת או לצלב קישור לכל אירוע/שגיאה קונקרטיים, אמור את המילה ואני ארחיב אותה.