<!-- Hebrew translation of docs/source/data_model_and_isi_spec.md -->

---
lang: he
direction: rtl
source: docs/source/data_model_and_isi_spec.md
status: complete
translator: manual
---

<div dir="rtl">

# דגם הנתונים ו-ISI של Iroha v2 — מפרט שנגזר מהיישום

מסמך זה נגזר לאחור מהיישום הנוכחי ב־`iroha_data_model` וב־`iroha_core` ומיועד לסייע בבדיקת תכן. נתיבים המופיעים בסוגריים קדמיים מצביעים על קוד המקור הסמכותי.

## היקף
- מגדיר את היישויות הקנוניות (דומיינים, חשבונות, נכסים, NFT, תפקידים, הרשאות, פירז, טריגרים) ואת מזהיהם.
- מתאר הוראות משנה-מצב (ISI): סוגים, פרמטרים, תנאי קדם, מעברי מצב, אירועים נפלטים ותנאי שגיאה.
- מסכם ניהול פרמטרים, טרנזקציות וסריאליזציה של הוראות.

דטרמיניזם: כל סמנטיקת ההוראות היא מעבר מצב טהור שאינו תלוי בחומרה. סריאליזציה נעשית באמצעות Norito; בייטקוד VM משתמש ב-IVM ועובר ולידציה בצד ההוסט לפני ביצוע על השרשרת.

---

## יישויות ומזהים
למזהים צורות מחרוזת יציבות המאפשרות Round-trip בין `Display` ל-`FromStr`. חוקי שמות אוסרים רווחים ואת התווים השמורים `@ # $`.

- `Name` — מזהה טקסטואלי מאומת. חוקים: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. דומיין: `{ id, logo, metadata, owned_by }`. Builder: ‏`NewDomain`. קוד: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — הכתובת הקנונית נוצרת ע"י `AccountAddress` (קודק IH58 / תצוגת סורא הדחוסה `snx1…` / hex). מחרוזות `alias@domain` נשמרות ככינויי ניתוב בלבד. Torii מנרמל קלטים דרך `AccountAddress::parse_any` כך שהמטא־דאטה נשמר בפורמט הקנוני. חשבון: `{ id, metadata }`. קוד: `crates/iroha_data_model/src/account.rs`.
- `AssetDefinitionId` — `asset#domain`. הגדרה: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. קוד: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId` — `asset#domain#account@domain` או `asset##account@domain` כאשר הדומיינים תואמים. נכס: `{ id, value: Numeric }`. קוד: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`.
- `NftId` — `nft$domain`. ‏NFT: ‏`{ id, content: Metadata, owned_by }`. קוד: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. תפקיד: `{ id, permissions: BTreeSet<Permission> }` עם Builder ‏`NewRole { inner: Role, grant_to }`. קוד: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. קוד: `crates/iroha_data_model/src/permission.rs`.
- `PeerId` / `Peer` — מפתח ציבורי וכתובת של פיֵר. קוד: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. טריגר: `{ id, action }`. פעולה: `{ executable, repeats, authority, filter, metadata }`. קוד: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — ‏`BTreeMap<Name, Json>` עם בדיקות בהוספה/הסרה. קוד: `crates/iroha_data_model/src/metadata.rs`.

תכונות חשובות: ‏`Identifiable`, ‏`Registered`/‏`Registrable` (דפוס Builder), ‏`HasMetadata`, ‏`IntoKeyValue`. קוד: `crates/iroha_data_model/src/lib.rs`.

אירועים: כל יישות מפיקה אירועים בעת שינוי (יצירה/מחיקה/שינוי בעלות/שינוי מטה־דאטה וכו׳). קוד: `crates/iroha_data_model/src/events/`.

---

## פרמטרים (קונפיגורציית שרשרת)
- משפחות: ‏`SumeragiParameters { block_time_ms, commit_time_ms, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, ‏`BlockParameters { max_transactions }`, ‏`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, ‏`SmartContractParameters { fuel, memory, execution_depth }`, וכן ‏`custom: BTreeMap`.
- enums בודדים לשינויים: ‏`SumeragiParameter`, ‏`BlockParameter`, ‏`TransactionParameter`, ‏`SmartContractParameter`. אגרגטור: ‏`Parameters`. קוד: `crates/iroha_data_model/src/parameter/system.rs`.

הגדרת פרמטרים (ISI): ‏`SetParameter(Parameter)` מעדכן את השדה המתאים ופולט `ConfigurationEvent::Changed`. קוד: `crates/iroha_data_model/src/isi/transparent.rs`, יישום באקזקיוטור: `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## סריאליזציית הוראות והרשמה
- תכונת ליבה: ‏`Instruction: Send + Sync + 'static` עם `dyn_encode()`, ‏`as_any()`, ‏`id()` יציב (ברירת המחדל היא שם הטיפוס הקונקרטי).
- ‏`InstructionBox`: מעטפת של ‏`Box<dyn Instruction>`. ‏`Clone` / ‏`Eq` / ‏`Ord` פועלים על `(type_id, encoded_bytes)` לכן שוויון הוא לפי ערך.
- מזהי Wire יציבים: הרגיסטרי תומך ב“wire IDs” לפי סוג, המשמשים בסריאליזציה במקום `type_name` של Rust לצורך הפרדה מהטמעה. ברגיסטרי ברירת המחדל יש מזהים מפורשים להוראות נפוצות (למשל `Log` → ‏`iroha.log`, ‏`SetParameter` → ‏`iroha.set_parameter`, ‏`ExecuteTrigger` → ‏`iroha.execute_trigger`). בעת הדסיריאליזציה מתקבלים הן ה-wire ID והן ה-`type_name` לטובת תאימות לאחור.
- ‏Norito serde עבור `InstructionBox` מסריאליזת כ-`(String wire_id, Vec<u8> payload)` (נופל ל-`type_name` אם אין wire ID). דסיריאליזציה משתמשת ב-`InstructionRegistry` גלובלי שממפה מזהים לבנאים. ברירת המחדל כוללת את כל ה-ISI המובנים. קוד: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: סוגים, סמנטיקה, שגיאות
הביצוע ממומש באמצעות `Execute for <Instruction>` ב-`iroha_core::smartcontracts::isi`. להלן מובאים האפקטים הגלויים, תנאי הקדם, האירועים והשגיאות.

### רישום / ביטול רישום
סוגים: ‏`Register<T>` ו-`Unregister<T>` (‏`T ∈ { Domain, Account, AssetDefinition, Asset, Nft, Role, Permission, Peer, Trigger }`). קיימות מעטפות בוקסות (`RegisterBox` וכו').

- רישום דומיין: שומר את הדומיין כאשר ה־`DomainId` אינו קיים.
  - תנאי קדם: דומיין עם אותו מזהה אינו רשום.
  - אירועים: `DomainEvent::Created`. ‏`owned_by` נרשם כמנפיק הבקשה.
  - שגיאות: ‏`Repetition(Register, DomainId)` על כפילות. קוד: `core/.../isi/world.rs`.

- רישום חשבון: שומר חשבון לפי `AccountId`.
  - תנאי קדם: הדומיין קיים והחשבון אינו רשום.
  - אירועים: `AccountEvent::Created`.
  - שגיאות: ‏`FindError::Domain` (דומיין חסר), ‏`Repetition(Register, AccountId)` (כפילות). קוד: `core/.../isi/domain.rs`.

- רישום הגדרת נכס: שומר `AssetDefinition` ומכין את טפלון ה-mintability (בפרט `Once` → `Not` לאחר המינט הראשון).
  - תנאי קדם: ההגדרה אינה קיימת.
  - אירועים: `AssetDefinitionEvent::Created`.
  - שגיאות: ‏`Repetition(Register, AssetDefinitionId)`. קוד: `core/.../isi/domain.rs`.

- רישום NFT: שומר `Nft`.
  - תנאי קדם: הדומיין קיים; אין כפילות מזהים.
  - אירועים: `NftEvent::Created`.
  - שגיאות: ‏`Repetition(Register, NftId)`. קוד: `core/.../isi/nft.rs`.

- רישום תפקיד: בונה מתוך ‏`NewRole { inner, grant_to }` ושומר את `inner: Role` (הענקה מוקדמת מבוצעת דרך `grant_to`).
  - תנאי קדם: מזהה התפקיד אינו תפוס.
  - אירועים: `RoleEvent::Created`.
  - שגיאות: ‏`Repetition(Register, RoleId)`. קוד: `core/.../isi/world.rs`.

- רישום טריגר: מאחסן את הטריגר בקבוצה המתאימה לפי סוג המסנן.
  - תנאי קדם: אם המסנן אינו mintable חייבים להגדיר `action.repeats = Exactly(1)` אחרת מתקבל `MathError::Overflow`. מזהה כפול אסור.
  - אירועים: `TriggerEvent::Created(TriggerId)`.
  - שגיאות: ‏`Repetition(Register, TriggerId)`, ובכשל המרה/ולידציה ‏`InvalidParameterError::SmartContract(..)`. קוד: `core/.../isi/triggers/mod.rs`.

- ביטול רישום (פיֵר/דומיין/חשבון/הגדרת נכס/NFT/תפקיד/טריגר): מסיר את היישות ופולט אירועים. הסרות מדורגות נוספות:
  - דומיין: מסיר את כל החשבונות בדומיין, את התפקידים/הרשאות שלהם, את מוני רצף העסקאות, את תוויות החשבון ואת קשירות ה‑UAID; מוחק את נכסי החשבונות (כולל מטא־דאטה של נכסים). מסיר את כל הגדרות הנכסים בדומיין; מוחק NFT בדומיין וכן NFT שבבעלות החשבונות שהוסרו; ומוחק טריגרים שסמכות הדומיין שלהם תואמת. אירועים: `DomainEvent::Deleted` ועוד אירוע לכל פריט. שגיאות: `FindError::Domain`. קוד: `core/.../isi/world.rs`.
  - חשבון: מסיר הרשאות ותפקידים של החשבון, מוני רצף עסקאות, תוויות חשבון וקשירות UAID; מוחק נכסים בבעלות החשבון (כולל מטא־דאטה של נכסים) ו‑NFT בבעלותו; מוחק טריגרים שהסמכות שלהם היא אותו חשבון. אירועים: `AccountEvent::Deleted` וכן `NftEvent::Deleted` לכל NFT שנמחק. שגיאות: `FindError::Account`. קוד: `core/.../isi/domain.rs`.
  - הגדרת נכס: מוחק את כלל הנכסים מהסוג ואת מטא־דאטה של אותם נכסים. אירועים: `AssetDefinitionEvent::Deleted` ו-`AssetEvent::Deleted`. שגיאות: `FindError::AssetDefinition`. קוד: `core/.../isi/domain.rs`.
  - NFT: מוחק את ה-NFT. אירועים: `NftEvent::Deleted`. שגיאות: `FindError::Nft`. קוד: `core/.../isi/nft.rs`.
  - תפקיד: מסיר את התפקיד מכל החשבונות ואז מוחק את התפקיד עצמו. אירועים: `RoleEvent::Deleted`. שגיאות: `FindError::Role`. קוד: `core/.../isi/world.rs`.
  - טריגר: מסיר אם קיים; ניסיון כפול מחזיר `Repetition(Unregister, TriggerId)`. אירועים: `TriggerEvent::Deleted`. קוד: `core/.../isi/triggers/mod.rs`.

### מינט/שריפה
סוגים: ‏`Mint<O, D: Identifiable>` ו-`Burn<O, D: Identifiable>` (ממומשים גם ב־`MintBox`/`BurnBox`).

- נכסים מספריים: מעדכנים יתרות ואת `total_quantity` של ההגדרה.
  - תנאי קדם: הערך עונה על `AssetDefinition.spec()`. הרשות למינט תלויה ב-`mintable`:
    - `Infinitely`: מותר תמיד.
    - `Once`: מותר פעם אחת; המינט הראשון משנה ל-`Not` ומפיק `AssetDefinitionEvent::MintabilityChanged` ו-`AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`.
    - `Limited(n)`: מאפשר עד `n` הטבעות נוספות. כל מינט מפחית את המונה; כאשר הוא מגיע לאפס ההגדרה עוברת ל-`Not` ומפיקה את אותם אירועים כמו ב-`Once`.
    - `Not`: נכשל עם `MintabilityError::MintUnmintable`.
  - שינוי מצב: ביצירת יתרה חדשה בעת מינט; מחיקת יתרה כאשר ניצב‑0 לאחר שריפה.
  - אירועים: `AssetEvent::Added` / `AssetEvent::Removed`, ו־`AssetDefinitionEvent::MintabilityChanged` כאשר `Once` או `Limited(n)` מתרוקנים.
  - שגיאות: ‏`TypeError::AssetNumericSpec(Mismatch)`, ‏`MathError::Overflow`/‏`NotEnoughQuantity`. קוד: `core/.../isi/asset.rs`.

- ספירת חזרות של טריגר: משנה את `action.repeats`.
  - תנאי קדם: מינט דורש מסנן mintable; אין חריגה אריתמטית.
  - אירועים: `TriggerEvent::Extended` / `TriggerEvent::Shortened`.
  - שגיאות: ‏`MathError::Overflow` במינט לא חוקי; ‏`FindError::Trigger`. קוד: `core/.../isi/triggers/mod.rs`.

### העברה
סוג: ‏`Transfer<S: Identifiable, O, D: Identifiable>` (`TransferBox`).

- נכס מספרי: מחסרים מ-`AssetId` המקור ומוסיפים ליעד (אותה הגדרה, חשבון שונה). נכס מקור נמחק אם היתרה מתאפסת.
  - תנאי קדם: נכס מקור קיים; הערך עומד ב-`spec`.
  - אירועים: `AssetEvent::Removed` (מקור), ‏`AssetEvent::Added` (יעד).
  - שגיאות: ‏`FindError::Asset`, ‏`TypeError::AssetNumericSpec`, ‏`MathError::NotEnoughQuantity/Overflow`. קוד: `core/.../isi/asset.rs`.

- בעלות דומיין: מעדכנת את `Domain.owned_by`.
  - תנאי קדם: שני החשבונות קיימים והדומיין קיים.
  - אירועים: `DomainEvent::OwnerChanged`.
  - שגיאות: ‏`FindError::Account/Domain`. קוד: `core/.../isi/domain.rs`.

- בעלות הגדרת נכס: מעדכנת `AssetDefinition.owned_by`.
  - תנאי קדם: שני החשבונות וההגדרה קיימים; המקור הוא בעל ההגדרה.
  - אירועים: `AssetDefinitionEvent::OwnerChanged`.
  - שגיאות: ‏`FindError::Account/AssetDefinition`. קוד: `core/.../isi/account.rs`.

- בעלות NFT: מעדכנת `Nft.owned_by`.
  - תנאי קדם: שני החשבונות ו־NFT קיימים; המקור הוא הבעלים.
  - אירועים: `NftEvent::OwnerChanged`.
  - שגיאות: ‏`FindError::Account/Nft`, ‏`InvariantViolation` אם המקור אינו הבעלים. קוד: `core/.../isi/nft.rs`.

### מטה־דאטה: הגדרת/הסרת מפתח-ערך
סוגים: ‏`SetKeyValue<T>` ו-`RemoveKeyValue<T>` עם ‏`T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. קיימות עטיפות בוקסיות.

- Set: מכניסה או מחליפה ערך `Metadata[key] = Json(value)`.
- Remove: מסירה את המפתח; מחזירה שגיאה אם אינו קיים.
- אירועים: ‏`<Target>Event::MetadataInserted` / ‏`MetadataRemoved` עם הערכים הישנים/חדשים.
- שגיאות: ‏`FindError::<Target>` אם היעד אינו קיים; ‏`FindError::MetadataKey` כאשר מפתח להסרה אינו קיים. קוד: `crates/iroha_data_model/src/isi/transparent.rs` והאקסקיוטורים הספציפיים.

### הרשאות ותפקידים: הענק/שלול
סוגים: ‏`Grant<O, D>` ו-`Revoke<O, D>`, עם enums בוקסיים להענקה/שלילה של `Permission` או `Role` לחשבון, ולהענקה/שלילה של `Permission` לתפקיד.

- הענקת הרשאה לחשבון: מוסיפה `Permission` כל עוד איננה מובנית.
  - אירועים: `AccountEvent::PermissionAdded`.
  - שגיאות: `Repetition(Grant, Permission)` על כפילות. קוד: `core/.../isi/account.rs`.

- שלילת הרשאה מחשבון: מסירה אם קיימת.
  - אירועים: `AccountEvent::PermissionRemoved`.
  - שגיאות: `FindError::Permission`. קוד: `core/.../isi/account.rs`.

- הענקת תפקיד לחשבון: מוסיפה מיפוי `(account, role)` אם אינו קיים.
  - אירועים: `AccountEvent::RoleGranted`.
  - שגיאות: `Repetition(Grant, RoleId)`. קוד: `core/.../isi/account.rs`.

- שלילת תפקיד מחשבון: מסירה את המיפוי.
  - אירועים: `AccountEvent::RoleRevoked`.
  - שגיאות: `FindError::Role`. קוד: `core/.../isi/account.rs`.

- הענקת הרשאה לתפקיד: בונה מחדש את התפקיד עם ההרשאה החדשה.
  - אירועים: `RoleEvent::PermissionAdded`.
  - שגיאות: `Repetition(Grant, Permission)`. קוד: `core/.../isi/world.rs`.

- שלילת הרשאה מתפקיד: בונה מחדש את התפקיד ללא אותה הרשאה.
  - אירועים: `RoleEvent::PermissionRemoved`.
  - שגיאות: `FindError::Permission`. קוד: `core/.../isi/world.rs`.

### טריגרים: Execute
סוג: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- התנהגות: מניח בתור אירוע `ExecuteTriggerEvent { trigger_id, authority, args }` שהמערכת מנהלת הטריגרים צורכת. ביצוע ידני מותר רק לטריגרים עם פילטר `ExecuteTrigger` ‏(by-call); הפילטר חייב להתאים והקורא חייב להיות סמכות הפעולה של הטריגר או בעל הרשאת `CanExecuteTrigger` עבור אותה סמכות. כאשר יש executor שסופק ע״י המשתמש, הרצת הטריגר מאומתת ע״י executor בזמן ריצה וצורכת את תקציב ה-fuel של העסקה (בסיס `executor.fuel` + מטה־דאטה `additional_fuel`).
- שגיאות: `FindError::Trigger` אם אינו רשום; `InvariantViolation` אם מזוהה ע״י גורם שאינו הסמכות. קוד: `core/.../isi/triggers/mod.rs` (בדיקות ב-`core/.../smartcontracts/isi/mod.rs`).

### Upgrade ו-Log
- `Upgrade { executor }`: מעביר את האקזקיוטור באמצעות בייטקוד חדש, מעדכן את האקזקיוטור ודגם הנתונים ומפיק `ExecutorEvent::Upgraded`. כשל עטוף ב-`InvalidParameterError::SmartContract`. קוד: `core/.../isi/world.rs`.
- `Log { level, msg }`: מפיק לוג ברמת החומרה הניתנת; ללא שינוי מצב. קוד: `core/.../isi/world.rs`.

### מודל השגיאות
המעטפת היא `InstructionExecutionError` עם וריאנטים לשגיאות הערכה, כשלי שאילתה, המרות, יישות לא נמצאה, כפילות, mintability, שגיאות מתמטיות, פרמטר לא חוקי והפרת אינביריאנט. ההגדרות נמצאות ב-`crates/iroha_data_model/src/isi/mod.rs` תחת `pub mod error`.

---

## טרנזקציות וישויות ניתנות לביצוע
- `Executable`: או `Instructions(ConstVec<InstructionBox>)` או `Ivm(IvmBytecode)`; בייטקוד מסריאליזת כ-base64. קוד: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder` / `SignedTransaction`: בונה, חותם ואורז ישות ביצוע עם מטה־דאטה, `chain_id`, `authority`, ‏`creation_time_ms`, ‏`ttl_ms` אופציונלי ו-`nonce`. קוד: `crates/iroha_data_model/src/transaction/`.
- בזמן ריצה `iroha_core` מפעילה אצוות של `InstructionBox` דרך `Execute for InstructionBox` ומדרגה לטיפוס הרלוונטי. קוד: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- תקציב אימות executor בזמן ריצה (executor שסופק ע״י משתמש): `executor.fuel` מהפרמטרים ועוד `additional_fuel` ‏(`u64`) ממטה־הדאטה של העסקה, משותף לאימותי הוראות/טריגרים באותה עסקה.

---

## אינביריאנטים והערות (על סמך בדיקות ומגנים)
- הגנות Genesis: אי אפשר לרשום את הדומיין `genesis` או חשבונות בתוכו; חשבון `genesis` אינו ניתן להרשמה. קוד/בדיקות: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- נכסים מספריים חייבים לעמוד ב-`NumericSpec` בעת מינט/העברה/שריפה; אי התאמה → ‏`TypeError::AssetNumericSpec`.
- Mintability: ‏`Once` מאפשר מינט יחיד ולאחר מכן עובר ל-`Not`; ‏`Limited(n)` מאפשר בדיוק `n` הטבעות נוספות ולאחר מכן עובר ל-`Not`. ניסיון לאסור מינט על `Infinitely` מפיק `MintabilityError::ForbidMintOnMintable`, והגדרה של `Limited(0)` מחזירה `MintabilityError::InvalidMintabilityTokens`. יש Guard בעזרי ה-ISI של חשבון.
- פעולות מטה־דאטה מדויקות לפי מפתח; ניסיון להסיר מפתח שאינו קיים הוא שגיאה.
- פילטרים שאינם mintable מחייבים את `Register<Trigger>` לקבל `Exactly(1)` חזרות.
- דטרמיניזם: כל האריתמטיקה מבוצעת עם בדיקת חריגה; Overflow/Underflow מחזירים שגיאות מתמטיות טיפוסיות; יתרות אפסיות מוסרות ולכן אין “מצב נסתר”.

---

## דוגמאות מעשיות
- מינט והעברה:
  - `Mint::asset_numeric(10, asset_id)` → מוסיף 10 אם ה-spec וה-mintability מאפשרים; אירוע: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → מעביר 5; אירועי הסרה/הוספה נלווים.
- עדכוני מטה־דאטה:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; הסרה עם `RemoveKeyValue::account(...)`.
- ניהול תפקידים/הרשאות:
  - `Grant::account_role(role_id, account)`, ‏`Grant::role_permission(perm, role)` והגרסאות `Revoke`.
- מחזור חיי טריגר:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` (המסנן רומז על mintability); ‏`ExecuteTrigger::new(id).with_args(&args)` חייב להתאים לסמכות המוגדרת.
- עדכון פרמטר:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` מעדכן ופולט `ConfigurationEvent::Changed`.

---

## עקיבות (מקורות נבחרים)
- ליבת דגם הנתונים: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
- הגדרות ISI ורגיסטרי: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
- ביצוע ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
- אירועים: `crates/iroha_data_model/src/events/**`.
- טרנזקציות: `crates/iroha_data_model/src/transaction/**`.

אם תרצו להרחיב את המפרט לטבלה מעובדת של API/התנהגות או להצליב לכל אירוע/שגיאה קונקרטיים, ספרו לי ואוסיף זאת.

</div>
