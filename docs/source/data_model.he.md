<!-- Hebrew translation of docs/source/data_model.md -->

---
lang: he
direction: rtl
source: docs/source/data_model.md
status: complete
translator: manual
---

<div dir="rtl">

# Iroha v2 – מודל הנתונים

המסמך מפרט את המבנים, המזהים, הטרייטים והפרוטוקולים של הקרייט `iroha_data_model` כפי שנעשה בהם שימוש ברחבי ה-workspace.

## תחום ותשתית

- **מטרה:** לספק טיפוסים קנוניים לישויות ליבה (דומיינים, חשבונות, נכסים, NFT, תפקידים, הרשאות, עמיתים), הוראות משנה מצב (ISI), שאילתות, טריגרים, טרנזקציות, בלוקים ופרמטרים.
- **סריאליזציה:** כל הטיפוסים הציבוריים מממשים את Norito (`Encode`, `Decode`) ואת `IntoSchema`. JSON זמין תלוית-פיצ׳ר.
- **הערת IVM:** בחלק מהטיפוסים נטרלות בדיקות בזמן דסיריאליזציה עבור IVM, משום שה-host מאמת לפני הרצת חוזה.
- **FFI:** `iroha_ffi` מאופשר via `ffi_export`/`ffi_import` כדי להימנע מהתקורה ללא FFI.

## טרייטים עיקריים

- `Identifiable`: מחזיר מזהה יציב. מומלץ `IdEqOrdHash` עבור שימוש במבני map/set.
- `Registrable` / `Registered`: דפוס builder עבור ישויות הרשמה (`Domain`, `AssetDefinition`, `Role` וכו').
- `HasMetadata`: גישה אחידה ל-`Metadata` (מפת Name→Json).
- `IntoKeyValue`: מסייע בפיצול `Key`/`Value` באחסון.
- `Owned<T>` / `Ref<'world, K, V>`: עטיפות קלות שמפחיתות העתקות.

## שמות ומזהים

- `Name`: מזהה טקסטואלי חוקי (ללא `@`, `#`, `$`, מנורמל ל-NFC). `genesis` שמור.
- `IdBox`: מעטפת לאוסף מזהים (DomainId, AccountId, ...).
- `ChainId`: לזיהוי שרשרת ולמניעת Replay.

**ייצוגים טקסטואליים:**
- `DomainId`: `wonderland`
- `AccountId`: הכתובת הקנונית מנוהלת דרך `AccountAddress` עם קודקים ל‑IH58, תצוגת סורה הדחוסה (`sora…`) והקסדצימלית (`canonical_hex`). IH58 הוא הפורמט המועדף, ו־`sora…` הוא אפשרות שנייה ייעודית ל‑Sora. מחרוזות `alias` (rejected legacy form) נשמרות ככינויי ניתוב בלבד. Torii מנרמל קלטים דרך `AccountAddress::parse_encoded`. Supports single-key and multisig controllers.
- `AssetDefinitionId`: `asset#domain`
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain`
- `PeerId`: מפתח פומבי

## ישויות

- **Domain**: מזהה, לוגו, מטא-דאטה, בעלים.
- **Account**: מזהה, מטא-דאטה.
- **AssetDefinition**: מזהה, Spec, Mintable, לוגו, מטא-דאטה, בעלים, כמות.
- **Asset**: מזהה, ערך.
- **NFT**: מזהה, תוכן (Metadata), בעלים.
- **Role**: מזהה, סט הרשאות.
- **Permission**: שם (`Ident`) ומטען `Json` בהתאם ל-`ExecutorDataModel`.
- **Peer**: כתובת ומזהה.
- **Trigger**: מזהה ו-`action::Action`.

## Metadata

`Metadata` הוא `BTreeMap<Name, Json>` עבור דומיין/חשבון/AssetDefinition/Nft/טריגרים/טרנזקציות.

## הוראות ו-ISI

- `InstructionBox` – כל ההוראות (`Register`, `GrantRole`, `MintAsset`, ...).
- `CustomInstruction` – ללוגיקה מותאמת ברמת ה-Executor (לא מומלץ ברשת ציבורית).

## שאילתות

- `QueryBox`: שאילתות יחידניות וארוכות (`FindTransactions`, `FindDomainById` וכו').
- `QueryRequest`: `Singular` / `Start` / `Continue`; `QueryResponse`: `Singular` / `Iterable`.
- DSL (`query::dsl`) מספק בדיקת predicates/Selectors בזמן קומפילציה (`fast_dsl` לפונקציונליות קלה יותר).

## טרנזקציות ובלוקים

- `Transaction`/`VersionedTransaction`: nonce/TTL/חתימות.
- `SignedTransaction`: מזהה שרשרת ומסלול חתימה.
- `BlockHeader`/`Block`: מטא-דאטה של בלוק ורשימת טרנזקציות.
- `SignedBlock`: כולל `signatures: BTreeSet<BlockSignature>` של הוולידטורים, `BlockPayload { header, transactions }` ו-`BlockResult`. בתוך `BlockResult` נשמרים `time_triggers`, עצי מרקל לכניסות/תוצאות, `transaction_results`, וכן `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` כדי לשמר את תמלילי ה-FASTPQ שנאספו במהלך ההרצה.
- הודעות `ExecWitness` המשודרות ב-Torii כוללות כעת בנוסף ל-`fastpq_transcripts` גם `fastpq_batches: Vec<FastpqTransitionBatch>` עם `public_inputs` (dsid, slot, old/new roots, perm_root, tx_set_hash), כך שמפעילי פרוברים יכולים למשוך את שורות ה-FASTPQ הקנוניות בלי להמיר את התמלילים מחדש.
- כלים: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature()`, `replace_signatures()`.

## אירועים

- `DataEvent` ו-`PipelineEvent` מכסים אירועי נתונים וצינור.

## טריגרים

`Trigger { action::Action }` – פעולות מתוזמנות או מונעות נתונים.

## פרמטרים

`Parameter` ו-`CustomParameter` מנהלים קונפיגורציות רשת; `CustomParameters` עבור הרחבות Executor.

## Executor והרחבה

- `Executor { bytecode }`: הבייטקוד שהולידטורים מריצים.
- `ExecutorDataModel` מקטלגת פרמטרים, הוראות והרשאות מותאמות.
- `CustomInstruction` מציע מעטפת להוראות מותאמות; רצוי להימנע ברשתות ציבוריות.

## פיצ׳רים ודטרמיניזם

- פיצ׳רים: `std`, `json`, `transparent_api`, `ffi_export/import`, `fast_dsl`, `http`, `fault_injection`.
- Norito מבטיח תוצאה דטרמיניסטית על פני חומרות. IVM חייב להפעיל בייטקוד באופן זהה בכל הצמתים.

### API שקוף (`transparent_api`)

- מטרה: לחשוף גישה מלאה וברת שינוי למבני `#[model]` עבור רכיבים פנימיים כמו Torii, מפעילים ומבחני אינטגרציה. כשהפיצ׳ר כבוי הפריטים נשארים אטומים ו-SDK חיצוניים עובדים רק עם בנאים בטוחים וייצוגים מקודדים.
- מנגנון: המאקרו `iroha_data_model_derive::model` מצמיד לכל שדה ציבורי את הצמדי `#[cfg(feature = "transparent_api")] pub` ומשאיר גרסה פרטית לברירת המחדל. הפעלת הפיצ׳ר מחליפה את ה-cfg וכך ניתן לפרק `Account`, `Domain`, `Asset` ועוד מחוץ למודולים שלהם.
- זיהוי: הקרייט מייצא קבוע `TRANSPARENT_API: bool` (בקבצים המחוללים `transparent_api.rs` או `non_transparent_api.rs`). קוד תלוי יכול לבדוק את הדגל ולבחור fallback אטום בעת הצורך.
- הפעלה: הוסיפו `features = ["transparent_api"]` לתלות ב-`Cargo.toml`. קרייטי ה-workspace שזקוקים להקרנת JSON (למשל `iroha_torii`) כבר מעבירים את הפיצ׳ר, אך צדדים שלישיים צריכים להשאיר אותו כבוי אלא אם הם שולטים בפריסה ומקבלים את שטח ה-API המורחב.

## דוגמאות

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/Mint/Transfer */ ])
    .sign(kp.private_key());
```

</div>
