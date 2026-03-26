<!-- Hebrew translation of docs/source/data_model.md -->

---
lang: he
direction: rtl
source: docs/source/data_model.md
status: complete
translator: machine-google-reviewed
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
---

# דגם נתונים Iroha v2 – Deep Dive

מסמך זה מסביר את המבנים, המזהים, התכונות והפרוטוקולים המהווים את מודל הנתונים Iroha v2, כפי שיושם בארגז `iroha_data_model` ומשמש ברחבי סביבת העבודה. זה נועד להוות התייחסות מדויקת שתוכל לעיין בה ולהציע עדכונים.

## היקף ויסודות

- מטרה: לספק סוגים קנוניים לאובייקטי דומיין (דומיינים, חשבונות, נכסים, NFTs, תפקידים, הרשאות, עמיתים), הוראות לשינוי מצב (ISI), שאילתות, טריגרים, טרנזקציות, חסימות ופרמטרים.
- סדרה: כל הסוגים הציבוריים נובעים מ-Codec Norito (`norito::codec::{Encode, Decode}`) וסכימה (`iroha_schema::IntoSchema`). נעשה שימוש ב-JSON באופן סלקטיבי (למשל, עבור עומסי HTTP ו-`Json`) מאחורי דגלי תכונה.
- הערה IVM: אימותים מסוימים בזמן דה-סריאליזציה מושבתים בעת מיקוד למכונה הווירטואלית Iroha (IVM), מכיוון שהמארח מבצע אימות לפני הפעלת חוזים (ראה מסמכי ארגז ב-I100NI3600).
- שערי FFI: סוגים מסוימים מסומנים על תנאי עבור FFI דרך `iroha_ffi` מאחורי `ffi_export`/`ffi_import` כדי למנוע תקורה כאשר אין צורך ב-FFI.

## תכונות ליבה ועוזרים- `Identifiable`: לישויות יש `Id` ו-`fn id(&self) -> &Self::Id` יציב. צריך להיות נגזר עם `IdEqOrdHash` עבור ידידותיות למפה/סט.
- `Registrable`/`Registered`: ישויות רבות (למשל, `Domain`, `AssetDefinition`, `Role`) משתמשות בדפוס בונה. `Registered` קושר את סוג זמן הריצה לסוג בונה קל משקל (`With`) המתאים לעסקאות רישום.
- `HasMetadata`: גישה אחידה למפת מפתח/ערך `Metadata`.
- `IntoKeyValue`: עוזר פיצול אחסון לאחסון `Key` (מזהה) ו-`Value` (נתונים) בנפרד כדי להפחית כפילות.
- `Owned<T>`/`Ref<'world, K, V>`: עטיפות קלות משקל המשמשות באחסון ומסנני שאילתות כדי למנוע עותקים מיותרים.

## שמות ומזהים- `Name`: מזהה טקסטואלי חוקי. לא מאפשר רווח לבן ותווים שמורים `@`, `#`, `$` (בשימוש במזהים מורכבים). ניתן לבנייה באמצעות `FromStr` עם אימות. שמות מנורמלים ל-Unicode NFC בניתוח (איותים מקבילים מבחינה קנונית מטופלים כאל זהים ומאוחסנים מורכבים). השם המיוחד `genesis` שמור (מסומן ללא רגישות רישיות).
- `IdBox`: מעטפה מסוג סכום עבור כל מזהה נתמך (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, Kotodama00006NI90, Kotodama0000690, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). שימושי עבור זרימות גנריות וקידוד Norito כסוג יחיד.
- `ChainId`: מזהה שרשרת אטום המשמש להגנה על שידור חוזר בעסקאות.מחרוזת צורות של מזהים (ניתנים להליכה הלוך ושוב עם `Display`/`FromStr`):
- `DomainId`: `name` (לדוגמה, `wonderland`).
- `AccountId`: מזהה חשבון קנוני ללא דומיין מקודד באמצעות `AccountAddress` כ-I105 בלבד. כניסות מנתח חייבות להיות קנוניות I105; סיומות תחום (`@domain`), מילוליות I105 קנוניות, מילוליות כינוי, קלט קנוני של מנתח hex, מטענים מדור קודם של `norito:` ו-`uaid:`/Norito הם טפסי חשבונות נדחים.
- `AssetDefinitionId`: `unprefixed Base58 address with versioning and checksum` קנוני (UUID-v4 בתים).
- `AssetId`: מילולית מקודדת קנונית `<asset-definition-id>#<i105-account-id>` (טפסים טקסטואליים מדור קודם אינם נתמכים במהדורה הראשונה).
- `NftId`: `nft$domain` (לדוגמה, `rose$garden`).
- `PeerId`: `public_key` (שוויון עמיתים הוא לפי מפתח ציבורי).

## ישויות

### דומיין
- `DomainId { name: Name }` - שם ייחודי.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Builder: `NewDomain` עם `with_logo`, `with_metadata`, ואז `Registrable::build(authority)` ערכות `owned_by`.### חשבון
- `AccountId` היא זהות החשבון הקנונית ללא דומיין הממוקמת על ידי הבקר ומקודדת כ-I105 הקנונית.
- `ScopedAccountId { account: AccountId, domain: DomainId }` נושא הקשר מפורש של תחום רק כאשר נדרשת תצוגה בהיקף.
- `Account { id, metadata, label?, uaid? }` — `label` הוא כינוי יציב אופציונלי המשמש רשומות מפתח מחדש, `uaid` נושא את האופציונלי Nexus רחב [מזהה חשבון אוניברסלי](Norito).
- Builder: `NewAccount` דרך `Account::new(id)`; הרישום מחייב דומיין `ScopedAccountId` מפורש ואינו מסיק כזה מברירות מחדל.

### הגדרות ונכסים של נכסים
- `AssetDefinitionId { aid_bytes: [u8; 16] }` חשוף טקסטואלית כ-`unprefixed Base58 address`.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` נדרש טקסט תצוגה הפונה לאדם ואסור להכיל `#`/`@`.
  - `alias` הוא אופציונלי וחייב להיות אחד מ:
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    עם הפלח השמאלי תואם בדיוק ל-`AssetDefinition.name`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - בונים: `AssetDefinition::new(id, spec)` או נוחות `numeric(id)`; `name` נדרש ויש להגדיר אותו באמצעות `.with_name(...)`.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` עם `AssetEntry`/`AssetValue` ידידותי לאחסון.
- `AssetBalanceScope`: `Global` עבור יתרות בלתי מוגבלות ו-`Dataspace(DataSpaceId)` עבור יתרות מוגבלות במרחב נתונים.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` חשוף עבור ממשקי API לסיכום.### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (התוכן הוא מטא נתונים שרירותיים של מפתח/ערך).
- Builder: `NewNft` דרך `Nft::new(id, content)`.

### תפקידים והרשאות
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` עם בונה `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` - סכימת `name` וסכימת המטען חייבים להתיישר עם `ExecutorDataModel` הפעיל (ראה להלן).

### עמיתים
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` וצורת מחרוזת `public_key@address` ניתנת לניתוח.

### פרימיטיבים קריפטוגרפיים (תכונה `sm`)
- `Sm2PublicKey` ו-`Sm2Signature`: נקודות תואמות SEC1 וחתימות `r∥s` ברוחב קבוע עבור SM2. בנאים מאמתים חברות בעקומה ומזהות הבחנה; קידוד Norito משקף את הייצוג הקנוני המשמש את `iroha_crypto`.
- `Sm3Hash`: `[u8; 32]` חדש המייצג את תקציר GM/T 0004, בשימוש במניפסטים, טלמטריה ותגובות מערכתיות.
- `Sm4Key`: מעטפת מפתח סימטרית של 128 סיביות משותף בין מערכות מערכות מארח ותקני מודל נתונים.
סוגים אלה יושבים לצד הפרימיטיבים הקיימים של Ed25519/BLS/ML-DSA והופכים לחלק מהסכמה הציבורית ברגע שמרחב העבודה נבנה עם `--features sm`.### טריגרים ואירועים
- `TriggerId { name: Name }` ו-`Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` או `Exactly(u32)`; כלי עזר להזמנה ודלדול כלולים.
  - בטיחות: לא ניתן להשתמש ב-`TriggerCompleted` כמסנן של פעולה (מאומת במהלך (דה) הסדרה).
- `EventBox`: סוג סכום עבור אירועי צינור, אצווה של צינור, נתונים, זמן, הפעלה-טריגר ואירועים שהושלמו; `EventFilterBox` משקף את זה עבור מנויים ומסנני טריגר.

## פרמטרים ותצורה

- משפחות פרמטרים של מערכת (כל `Default`ed, קבלנים לשאת, והמרה לרשומות בודדות):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` מקבץ את כל המשפחות ו-`custom: BTreeMap<CustomParameterId, CustomParameter>`.
- רשימות של פרמטר בודד: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` עבור עדכונים ואיטרציה דמויי הבדל.
- פרמטרים מותאמים אישית: מוגדר מבצע, נישא כ-`Json`, מזוהה על-ידי `CustomParameterId` (a `Name`).

## ISI (Iroha הוראות מיוחדות)- תכונת ליבה: `Instruction` עם `dyn_encode`, `as_any`, ומזהה יציב לכל סוג `id()` (ברירת המחדל של שם סוג הבטון). כל ההוראות הן `Send + Sync + 'static`.
- `InstructionBox`: עטיפה `Box<dyn Instruction>` בבעלות עם שיבוט/eq/ord מיושם באמצעות מזהה סוג + בתים מקודדים.
- משפחות הדרכה מובנות מאורגנות תחת:
  - `mint_burn`, `transfer`, `register`, ו-`transparent` צרור עוזרים.
  - הקלד סכומים עבור מטא תזרימי: `InstructionType`, סכומים בקופסה כמו `SetKeyValueBox` (דומיין/חשבון/asset_def/nft/trigger).
- שגיאות: מודל שגיאה עשיר תחת `isi::error` (שגיאות בסוג הערכה, מצא שגיאות, יכולת טבעה, מתמטיקה, פרמטרים לא חוקיים, חזרות, אינוריאנטים).
- רישום הוראות: המאקרו `instruction_registry!{ ... }` בונה רישום פענוח בזמן ריצה המבוסס על שם סוג. בשימוש על ידי שיבוט `InstructionBox` ו-Norito כדי להשיג (דה) סדרה דינמית. אם שום רישום לא הוגדר במפורש דרך `set_instruction_registry(...)`, רישום ברירת מחדל מובנה עם כל הליבה של ISI מותקן בעצלתיים בשימוש הראשון כדי לשמור על יציבות בינאריות.

## עסקאות- `Executable`: או `Instructions(ConstVec<InstructionBox>)` או `Ivm(IvmBytecode)`. `IvmBytecode` מופיע בסידרה כ-base64 (סוג חדש שקוף מעל `Vec<u8>`).
- `TransactionBuilder`: בונה מטען עסקאות עם `chain`, `authority`, `creation_time_ms`, אופציונלי `time_to_live_ms` ו-`ScopedAccountId`, I010002110X, an `Executable`.
  - עוזרים: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, Kotodama, I012NI0000.
- `SignedTransaction` (בגרסה עם `iroha_version`): נושא `TransactionSignature` ומטען; מספק hashing ואימות חתימה.
- נקודות כניסה ותוצאות:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` עם עוזרי גיבוב.
  - `ExecutionStep(ConstVec<InstructionBox>)`: אצווה אחת של הוראות בהזמנה בעסקה.

## בלוקים- `SignedBlock` (בגרסה) מכילה:
  - `signatures: BTreeSet<BlockSignature>` (מאת אימות),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (מצב ביצוע משני) המכיל `time_triggers`, עצי כניסה/תוצאה Merkle, `transaction_results` ו-`fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- כלי עזר: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `header()`, `signatures()`, `hash()`, I10000040X.
- שורשי מרקל: נקודות כניסה ותוצאות של עסקאות מבוצעות באמצעות עצי מרקל; תוצאה שורש Merkle ממוקם בכותרת הבלוק.
- הוכחות הכללה בלוק (`BlockProofs`) חושפות הן הוכחות כניסה/תוצאה של Merkle והן את מפת `fastpq_transcripts` כך שמוכיחים מחוץ לשרשרת יכולים להביא את דלתות ההעברה הקשורות ל-hash של עסקה.
- הודעות `ExecWitness` (מוזרמות דרך Torii ומבוססות על רכילות קונצנזוס) כוללות כעת הן `fastpq_transcripts` והן `fastpq_batches: Vec<FastpqTransitionBatch>` מוכנות להוכחה עם `fastpq_batches: Vec<FastpqTransitionBatch>` מוטבעים עם `fastpq_batches: Vec<FastpqTransitionBatch>` משובצים (roots, חריצים, roots, perm_root, tx_set_hash), כך שמוכיחים חיצוניים יכולים להטמיע שורות FASTPQ קנוניות ללא קידוד מחדש של תמלילים.

## שאילתות- שני טעמים:
  - יחיד: יישם את `SingularQuery<Output>` (לדוגמה, `FindParameters`, `FindExecutorDataModel`).
  - ניתן לחזרה: יישם `Query<Item>` (לדוגמה, `FindAccounts`, `FindAssets`, `FindDomains` וכו').
- טפסים שנמחקו מהסוג:
  - `QueryBox<T>` הוא `Query<Item = T>` ארוז ומחוק עם שרת Norito מגובה ברישום גלובלי.
  - `QueryWithFilter<T> { query, predicate, selector }` משלב שאילתה עם פרדיקט/בורר DSL; ממיר לשאילתה ניתנת לחזרה שנמחקה באמצעות `From`.
- רישום וקודקים:
  - `query_registry!{ ... }` בונה רישום גלובלי למיפוי סוגי שאילתות קונקרטיות לבנאים לפי שם סוג עבור פענוח דינמי.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` ו-`QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` הוא סוג סכום מעל וקטורים הומוגניים (לדוגמה, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), בתוספת עוזרי tuple והרחבות יעילים.
- DSL: מיושם ב-`query::dsl` עם תכונות הקרנה (`HasProjection<PredicateMarker>` / `SelectorMarker`) עבור פרדיקטים ובוררים שנבדקו בזמן הידור. תכונת `fast_dsl` חושפת גרסה קלה יותר במידת הצורך.

## ביצוע והרחבה- `Executor { bytecode: IvmBytecode }`: חבילת הקוד שבוצעה על ידי האימות.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` מכריז על התחום המוגדר על ידי המבצע:
  - פרמטרי תצורה מותאמים אישית,
  - מזהי הוראות מותאמים אישית,
  - מזהי אסימון הרשאה,
  - סכימת JSON המתארת סוגים מותאמים אישית עבור כלי לקוח.
- דוגמאות להתאמה אישית קיימות תחת `data_model/samples/executor_custom_data_model` המדגימות:
  - אסימון הרשאה מותאם אישית באמצעות `iroha_executor_data_model::permission::Permission` נגזר,
  - פרמטר מותאם אישית מוגדר כסוג שניתן להמרה ל-`CustomParameter`,
  - הוראות מותאמות אישית מסודרות לתוך `CustomInstruction` לביצוע.

### הוראה מותאמת אישית (ISI המוגדר על ידי מנהל)- סוג: `isi::CustomInstruction { payload: Json }` עם מזהה חוט יציב `"iroha.custom"`.
- מטרה: מעטפה להוראות ספציפיות למבצע ברשתות פרטיות/קונסורציום או ליצירת אב טיפוס, ללא מזלג מודל הנתונים הציבוריים.
- התנהגות ברירת מחדל של מבצע: ה-executor המובנה ב-`iroha_core` אינו מבצע את `CustomInstruction` ויפהל אם יתקל בו. מבצע מותאם אישית חייב להוריד את `InstructionBox` ל-`CustomInstruction` ולפרש באופן דטרמיניסטי את המטען על כל המאמתים.
- Norito: מקודד/מפענח באמצעות `norito::codec::{Encode, Decode}` עם סכימה כלולה; המטען `Json` מסודר באופן דטרמיניסטי. נסיעות הלוך ושוב יציבות כל עוד מרשם ההוראות כולל `CustomInstruction` (הוא חלק מרישום ברירת המחדל).
- IVM: Kotodama מקמפל לקוד בייט IVM (`.to`) והוא הנתיב המומלץ ללוגיקה של יישום. השתמש רק ב-`CustomInstruction` עבור הרחבות ברמת המבצע שעדיין לא ניתן לבטא ב-Kotodama. הבטח דטרמיניזם ובינאריים זהים של מבצעים על פני עמיתים.
- לא עבור רשתות ציבוריות: אין להשתמש עבור רשתות ציבוריות שבהן מוציאים לפועל הטרוגניים מסתכנים במזלגות קונצנזוס. העדיפו להציע ISI מובנה חדש במעלה הזרם כאשר אתם זקוקים לתכונות פלטפורמה.

## מטא נתונים- `Metadata(BTreeMap<Name, Json>)`: מאגר מפתח/ערך מחובר למספר ישויות (`Domain`, `Account`, `AssetDefinition`, `Nft`, טריגרים ועסקאות).
- API: `contains`, `iter`, `get`, `insert`, ו(עם `transparent_api`) `remove`.

## תכונות ודטרמיניזם

- כולל שליטה בממשקי API אופציונליים (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, I00300X, I10300X, I10300X, `ffi_export` `fault_injection`).
- דטרמיניזם: כל ההסדרה משתמשת בקידוד Norito כדי להיות ניידת על פני חומרה. קוד בתים IVM הוא כתם בתים אטום; אסור לביצוע להכניס הפחתות לא דטרמיניסטיות. המארח מאמת עסקאות ומספק תשומות ל-IVM באופן דטרמיניסטי.

### ממשק API שקוף (`transparent_api`)- מטרה: חשיפת גישה מלאה וניתנת לשינוי למבני `#[model]` עבור רכיבים פנימיים כגון Torii, מבצעים ומבחני אינטגרציה. בלעדיו, הפריטים האלה אטומים בכוונה, כך ש-SDK חיצוניים רואים רק בנאים בטוחים ומטענים מקודדים.
- מכניקה: המאקרו `iroha_data_model_derive::model` משכתב כל שדה ציבורי עם `#[cfg(feature = "transparent_api")] pub` ושומר עותק פרטי עבור בניית ברירת המחדל. הפעלת התכונה הופכת את ה-cfgs האלה, כך שהסרת המבנה של `Account`, `Domain`, `Asset` וכו' הופכת לחוקית מחוץ למודולים המגדירים שלהם.
- זיהוי משטח: הארגז מייצא קבוע `TRANSPARENT_API: bool` (נוצר ל-`transparent_api.rs` או ל-`non_transparent_api.rs`). קוד במורד הזרם יכול לבדוק את הדגל והענף הזה כאשר הוא צריך לחזור לעוזרים אטומים.
- הפעלה: הוסף את `features = ["transparent_api"]` לתלות ב-`Cargo.toml`. ארגזי סביבת עבודה הזקוקים להקרנת JSON (למשל, `iroha_torii`) מעבירים את הדגל באופן אוטומטי, אך צרכנים של צד שלישי צריכים לשמור אותו כבוי אלא אם כן הם שולטים בפריסה ומקבלים את משטח ה-API הרחב יותר.

## דוגמאות מהירות

צור דומיין וחשבון, הגדירו נכס ובנו עסקה עם הוראות:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

שאילתות חשבונות ונכסים עם ה-DSL:

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

השתמש בקוד בייט של חוזה חכם IVM:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / כינוי עזר מהיר (CLI + Torii):

```bash
# Register an asset definition with canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauﾛ1P... \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```הערת הגירה:
- מזהי `name#domain` ישנים בהגדרת נכס אינם מתקבלים בגרסה 1.
- מזהי נכסים עבור טביעה/צריבה/העברה נשארים קנוניים `<asset-definition-id>#<i105-account-id>`; בנה אותם עם:
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - או `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## גירסאות

- `SignedTransaction`, `SignedBlock` ו-`SignedQuery` הם מבנים קנוניים בקוד Norito. כל אחד מיישם את `iroha_version::Version` כדי להגדיר את המטען שלו עם גרסת ה-ABI הנוכחית (כיום `1`) כאשר מקודדים באמצעות `EncodeVersioned`.

## סקור הערות / עדכונים פוטנציאליים

- שאילתת DSL: שקול לתעד תת-קבוצה יציבה הפונה למשתמש ודוגמאות עבור מסננים/בוררים נפוצים.
- משפחות הוראות: הרחב מסמכים ציבוריים המפרטים את גרסאות ה-ISI המובנות שנחשפו על ידי `mint_burn`, `register`, `transfer`.

---
אם חלק כלשהו צריך יותר עומק (למשל, קטלוג ISI מלא, רשימת רישום שאילתות מלאה או שדות כותרות חסימה), הודע לי וארחיב את הסעיפים האלה בהתאם.