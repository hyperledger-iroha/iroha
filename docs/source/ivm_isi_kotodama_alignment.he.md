<!-- Hebrew translation of docs/source/ivm_isi_kotodama_alignment.md -->

---
lang: he
direction: rtl
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
translator: manual
---

<div dir="rtl">

# IVM ⇄ ISI ⇄ מודל נתונים ⇄ Kotodama — סקירת יישור

מסמך זה מבצע ביקורת על האופן שבו סט הפקודות של Iroha Virtual Machine ‏(IVM) וממשק ה-syscall שלו ממופים ל-Iroha Special Instructions ‏(ISI) ול-`iroha_data_model`, וכיצד Kotodama מקמפלת אל הערימה הזו. המטרה היא לזהות פערים קיימים ולהציע שיפורים מעשיים כך שכל ארבע השכבות יעבדו באופן דטרמיניסטי ונוח למפתחים.

הערה על יעד הבייטקוד: חוזי Kotodama מקומפלים לבייטקוד של IVM (`.to`). הם *לא* מכוונים ל-“risc5”/‏RISC-V כארכיטקטורה עצמאית. כל קידוד דמוי RISC-V שמוזכר כאן הוא חלק מפורמט הפקודות המעורב של IVM ונשאר פרט מימוש פנימי.

## היקף ומקורות
- IVM: הקבצים `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` והמסמכים תחת `crates/ivm/docs/*`.
- ISI/מודל נתונים: `crates/iroha_data_model/src/isi/*`,‏ `crates/iroha_core/src/smartcontracts/isi/*`, והמסמך `docs/source/data_model_and_isi_spec.md`.
- Kotodama: הקבצים `crates/kotodama_lang/src/*` והתיעוד ב-`crates/ivm/docs/*`.
- אינטגרציה ליבה: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

מונחים:
- “ISI” מתייחס לסוגי הוראות מובנות שמעדכנות את מצב העולם דרך המפעיל (למשל RegisterAccount,‏ Mint,‏ Transfer).
- “Syscall” מתייחס ל-IVM `SCALL` עם מספר בן 8 ביטים שמאציל את הפעולה להוסט עבור פעולות ספר חשבונות.

---

## המיפוי הקיים (כפי שממומש היום)

### פקודות IVM
- פקודות אריתמטיקה, זיכרון, בקרת זרימה, קריפטוגרפיה, וקטורים ועזרי ZK מוגדרות ב-`instruction.rs` ומומשו ב-`ivm.rs`. הן דטרמיניסטיות ומסתמכות על מסלולי האצה (SIMD/‏Metal/‏CUDA) עם נפילת חירום ל-CPU.
- גבול מערכת/הוסט עובר דרך `SCALL` (opcode ‏0x60). רשימת המספרים נמצאת ב-`syscalls.rs` ומכסה פעולות עולם (רישום/ביטול דומיינים, חשבונות, נכסים, פעולות mint/burn/transfer, תפקידי הרשאות, טריגרים) לצד עזרי מערכת (`GET_PRIVATE_INPUT`,‏ `COMMIT_OUTPUT`,‏ `GET_MERKLE_PATH` וכו').

### שכבת ההוסט
- הממשק `IVMHost::syscall(number, &mut IVM)` מוגדר ב-`host.rs`.
- `DefaultHost` מממש רק עזרי שאינם נוגעים בספר החשבונות (ניהול זיכרון, הגדלת heap, קלט/פלט, עזרי הוכחות ZK, גילוי תכונות) ואינו מבצע שינויי מצב.
- קיים `WsvHost` דמוי לצורך הדגמה בתוך `mock_wsv.rs` שממפה תת-קבוצה של פעולות נכס (Transfer/Mint/Burn) ל-WSV זעיר בזיכרון בעזרת מפות אד-הוק ראשי תיבות→ID ברגיסטרים x10..x13.

### ISI ומודל הנתונים
- סוגי ה-ISI המובנים והסמנטיקה שלהם ממומשים ב-`iroha_core::smartcontracts::isi::*` ומתועדים ב-`docs/source/data_model_and_isi_spec.md`.
- `InstructionBox` משתמש ברגיסטרי מזהים (“wire IDs”) יציבים ובקידוד Norito. ההרצה הנוכחית בקור מעבירה את ההוראות בנתיב הנייטיבי הזה.

### אינטגרציית IVM בליבה
- `State::execute_trigger(..)` משכפל את ה-IVM המטמון, מוסיף `CoreHost::with_accounts_and_args`, ואז קורא `load_program` ו-`run`.
- `CoreHost` מממש את `IVMHost`: syscalls המשנים מצב מפוענחים דרך פריסת TLV של ה-pointer ABI, מתורגמים ל-ISI (`InstructionBox`) ומתווספים לתור. כשה-VM מסיים, ההוסט מעביר את ה-ISI למפעיל הסטנדרטי כך והרשאות, אינווריאנטים, אירועים וטלמטריה נותרים זהים להרצה נייטיבית. Syscalls שלא נוגעים ב-WSV עדיין נשלחים ל-`DefaultHost`.
- `executor.rs` ממשיך להריץ ISI מובנים בנייטיב; מעבר המפעיל הוולידטורי עצמו ל-IVM נשאר עבודה עתידית.

### Kotodama → IVM
- קיימים רכיבי פרונטאנד (lexer,‏ parser,‏ סמנטיקה מינימלית,‏ IR,‏ הקצאת רגיסטרים).
- מחולל הקוד (`kotodama::compiler`) מפיק תת-קבוצה של הוראות IVM ומשתמש ב-`SCALL` לפעולות נכס:
  - `MintAsset` — מגדיר x10=account,‏ x11=asset,‏ x12=&NoritoBytes(Numeric) ומבצע `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/‏`TransferAsset` פועלים בדפוס דומה.
- הדגמות `koto_*_demo.rs` מציגות שימוש ב-`WsvHost` עם מיפוי זמני של שלמים ל-ID לטובת בדיקות מהירות.

---

## פערים וחוסר התאמות

1) **כיסוי ופריטי הקור-הוסט**  
   - מצב נוכחי: `CoreHost` כבר נמצא בליבה ומתרגם חלק ניכר מה-syscalls של הספר לחשבונות ל-ISI שרצים בנתיב הרגיל. הכיסוי עדיין חלקי (למשל חלק מהתפקידים/הרשאות/טריגרים הם stubs), ונדרשים מבחני התאמה כדי לוודא שה-ISI שבתור מייצרים אותו מצב/אירועים כמו ההרצה הנייטיבית.

2) **ממשק syscall לעומת שמות/כיסוי ב-ISI ובמודל הנתונים**  
   - NFTs: שמות ה-syscall עברו לפורמט הקנוני `SYSCALL_NFT_*`, כך שהם תואמים למודל הנתונים.
   - תפקידים/הרשאות/טריגרים: קיימת רשימת syscalls, אך אין מימוש התייחסות או טבלת מיפוי שמראה כיצד כל קריאה נקשרת ל-ISI קונקרטי בליבה.
   - פרמטרים/סמנטיקה: לחלק מה-syscalls אין תיעוד של קידוד פרמטרים (ID טיפוסי לעומת מצביע) או של משטר הגז; לעומת זאת ב-ISI הסמנטיקה מפורטת היטב.

3) **ABI להעברת נתונים מטיפוסים בין ה-VM להוסט**  
   - TLV של ה-pointer ABI מפוענחים כיום ב-`CoreHost` בעזרת `decode_tlv_typed` ומספקים נתיב דטרמיניסטי, אך Kotodama עדיין משתמשת במיפויי רגיסטרים גולמיים. חסר סט כלים רשמי ל-ABI הזה.

4) **תלות הדגמת Kotodama במיפויי ID של מספרים שלמים**  
   - `WsvHost` משתמש במיפויי מספר→ID אד-הוק. זה אינו ה-ABI הרשמי ויוצר מושגים שונים מהליבה. נדרש נתיב רשמי שבו מצביעים + קידוד Norito נשלחים אל ההוסט.

5) **פערי שמות ותיעוד**  
   - תיעוד ה-IVM מפרט את ה-syscalls, אך תיעוד מודל הנתונים/ISI אינו מחזיק את אותה טבלת מיפוי. גם README של Kotodama אינו מסביר את נתיב התאימות.

---

## הצעות לשיפור

### A. השלמת CoreHost והוספת טסטים
- להשלים את החיבור בין `CoreHost` ל-`StateTransaction` כך שכל ISI מובנים יוכלו להיקרא דרך ה-pointer ABI.
- להוסיף מבחני קצה-לקצה לכל ISI שמוודאים שהרצה דרך `InstructionBox` הנייטיבי וזהה דרך IVM מניבה אותו מצב, אירועים ושגיאות.
- להוסיף בדיקות “shadow mode” שמשוות בזמן הריצה בין ה-ISI שה-VM הוסיף לתור לבין ביצוע נייטיבי ומדגישות פערים.

### B. הגדרת ABI ומפרט syscall
- כמויות נכס הן `Numeric` ולכן מועברות כמצביעי NoritoBytes; גם טיפוסים מורכבים אחרים מועברים כמצביע.
- ליצור רפרנס שממפה כל `SYSCALL_*` ל-`InstructionBox` תואם.
- לקבע רשמית את מוסכמת ההחזרה (הצלחה: `x10=1`; כשל: `x10=0` ו/או `VMError::HostRejected { code }` לשגיאות חמורות).
- להטמיע את ה-ABI בקוד הגנרטור של Kotodama ובמבחני IVM ולהפסיק להשתמש במפות השלמים→ID.

### C. שימוש חוזר בקידוד Norito
- להכניס עזרי קידוד Norito משותפים (למשל `encode_account_id`,‏ `encode_asset_definition_id`) לשימוש בליבה/‏Kotodama/‏בדיקות.
- ✅ נוספו מבחני round-trip ל-pointer ABI (`crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs`) שמוודאים שמעבר VM זיכרון → הוסט → ISI → VM זיכרון נשאר דטרמיניסטי.

### D. איחוד שגיאות וגז
- להוסיף שכבת תרגום בהוסט שממפה `InstructionExecutionError::{Find, Repetition, InvariantViolation, Math, Type, Mintability, InvalidParameter}` לקודי `VMError` מדויקים או לפרוטוקול תוצאה מורחב (למשל `x10=0/1` ו-`VMError::HostRejected { code }`).
- להגדיר טבלת גז לסיסקולים בליבה, לשקף אותה בתיעוד IVM, ולהבטיח עלות צפויה לפי גודל הקלט ובאופן בלתי תלוי בפלטפורמה.

### E. דטרמיניזם ופרימיטיבים משותפים
- להשלים את איחוד עצי ה-Merkle (ראו roadmap) ולהסיר/להפוך ל-alias את `ivm::merkle_tree` ל-`iroha_crypto` עם אותם עלים/הוכחות.
- להשאיר את `SETVL`/‏`PARBEGIN`/‏`PAREND` כפקודות שמורות עד שיושלמו בדיקות דטרמיניזם מקצה לקצה ואסטרטגיית מתזמן דטרמיניסטית; לתעד ש-IVM מתעלם מהן כיום.
- להבטיח שמסלולי האצה מפיקים תוצאות זהות בית-אחר-בית; במקרים שבהם קשה להשיג זאת, לשים מאחורי דגל תכונה ולוודא שבדיקות משוות לנפילת CPU.

### F. חיבור קומפיילר Kotodama
- להרחיב את מחולל הקוד ל-ABI הרשמי (סעיף B) עבור מזהים ופרמטרים מורכבים ולהפסיק את מיפוי המספרים.
- להוסיף builtins שממפים ישירות ל-syscalls של ISI מעבר לנכסים (דומיינים/חשבונות/תפקידים/הרשאות/טריגרים) עם שמות ברורים.
- להוסיף בדיקות הרשאות בזמן קומפילציה ואופציונלית אנוטציות `permission(...)`; אם אין הוכחה סטטית — ליפול לשגיאות הוסט בזמן ריצה.
- להוסיף בדיקות יחידה ב-`crates/ivm/tests/kotodama.rs` שמקמפילות ומריצות חוזים קטנים מקצה לקצה עם הוסט בדיקה שמפענח ארגומנטים ב-Norito ומשנה WSV זמני.

### G. תיעוד ונוחות למפתחים
- לעדכן את `docs/source/data_model_and_isi_spec.md` עם טבלת המיפוי ומידע ה-ABI.
- להוסיף ב-`crates/ivm/docs/` מסמך “IVM Host Integration Guide” שמסביר איך לממש `IVMHost` מעל `StateTransaction` אמיתי.
- להבהיר ב-README ובתיעוד הקרייטים ש-Kotodama מכוונת לבייטקוד IVM `.to` ושסיסקולים הם הגשר למצב העולם.

---

## טבלת מיפוי מוצעת (טיוטה ראשונית)

תת-קבוצה מייצגת — להשלים ולהרחיב תוך כדי מימוש ההוסט.

- `SYSCALL_REGISTER_DOMAIN(id: ptr DomainId)` → ISI ‏`Register<Domain>`
- `SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId)` → ISI ‏`Register<Account>`
- `SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8)` → ISI ‏`Register<AssetDefinition>`
- `SYSCALL_MINT_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI ‏`Mint<Numeric, Asset>`
- `SYSCALL_BURN_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI ‏`Burn<Numeric, Asset>`
- `SYSCALL_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI ‏`Transfer<Asset>`
- `SYSCALL_TRANSFER_V1_BATCH_BEGIN()` / `SYSCALL_TRANSFER_V1_BATCH_END()` → ISI ‏`TransferAssetBatch` ‎(פתיחת/סגירת תחום באצ׳; כל רשומה מפורקת לקריאת `transfer_asset`)
- `SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes<TransferAssetBatch>)` → מאפשר להגיש אצווה מקודדת מראש בקריאה אחת
- `SYSCALL_NFT_MINT_ASSET(id: ptr NftId, owner: ptr AccountId)` → ISI ‏`Register<Nft>`
- `SYSCALL_NFT_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId, id: ptr NftId)` → ISI ‏`Transfer<Nft>`
- `SYSCALL_NFT_SET_METADATA(id: ptr NftId, content: ptr Metadata)` → ISI ‏`SetKeyValue<Nft>`
- `SYSCALL_NFT_BURN_ASSET(id: ptr NftId)` → ISI ‏`Unregister<Nft>`
- `SYSCALL_CREATE_ROLE(id: ptr RoleId, role: ptr Role)` → ISI ‏`Register<Role>`
- `SYSCALL_GRANT_ROLE(account: ptr AccountId, role: ptr RoleId)` → ISI ‏`Grant<Role>`
- `SYSCALL_REVOKE_ROLE(account: ptr AccountId, role: ptr RoleId)` → ISI ‏`Revoke<Role>`
- `SYSCALL_SET_PARAMETER(param: ptr Parameter)` → ISI ‏`SetParameter`

הערות:
- “`ptr T`” משמעו מצביע ברגיסטר אל בתים מקודדי Norito עבור T בזיכרון ה-VM; ההוסט מפענח אותם לטיפוס המתאים ב-`iroha_data_model`.
- מוסכמת החזרה: הצלחה — `x10=1`; כשל — `x10=0` ואפשרות לזריקת `VMError::HostRejected` לשגיאה פטאלית.

---

## סיכונים ותכנית השקה
- להתחיל בחיבור ההוסט לקבוצת פעולות צרה (נכסים + חשבונות) ולהוסיף בדיקות ממוקדות.
- להשאיר את ביצוע ה-ISI הנייטיבי כנתיב הסמכותי עד להבשלת הסמנטיקה בהוסט; להריץ את שני הנתיבים במצב “shadow” בבדיקות כדי לאמת תוצאות ואירועים זהים.
- אחרי אימות parity, לאפשר את ה-IVM host עבור טריגרי IVM בפרודקשן; בהמשך לשקול הפניית טרנזקציות רגילות דרך IVM.

---

## משימות להמשך
- לממש את `iroha_core::smartcontracts::ivm::host::CoreHost` שעוטף `StateTransaction` ו-`authority`.
- להוסיף ב-Kotodama עזרי encode/decode שמקבלים ארגומנטים מקודדי Norito דרך מצביעים.
- ✔ שמות ה-syscall של NFT הותאמו והן מתועדות בקוד ובמסמכים.
- להגדיר ולתעד טבלת גז לסיסקולים ולהחיל אותה בהוסט.
- להרחיב את הכיסוי ל-ABI המחודש של Kotodama לאחר חיווט העזרי encode/decode (סעיף B), על בסיס מבחני ה-round-trip הקיימים.

</div>
