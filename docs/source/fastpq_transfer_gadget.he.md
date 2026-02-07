---
lang: he
direction: rtl
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% עיצוב גאדג'ט העברת FastPQ

# סקירה כללית

מתכנן FASTPQ הנוכחי מתעד כל פעולה פרימיטיבית הכרוכה בהוראה `TransferAsset`, כלומר כל העברה משלמת עבור חשבון יתרה, סבבי גיבוב ועדכוני SMT בנפרד. כדי לצמצם שורות מעקב בכל העברה אנו מציגים גאדג'ט ייעודי המאמת רק את בדיקות החשבון/התחייבויות המינימליות בזמן שהמארח ממשיך לבצע את מעבר המצב הקנוני.

- **היקף**: העברות בודדות ואצוות קטנות הנפלטות דרך משטח ה-Sycall הקיים Kotodama/IVM `TransferAsset`.
- **יעד**: לחתוך את טביעת הרגל של עמודות FFT/LDE להעברות בנפחים גבוהים על ידי שיתוף טבלאות חיפוש וכיווץ אריתמטיקה לכל העברה לתוך בלוק אילוץ קומפקטי.

# אדריכלות

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## פורמט תמלול

המארח פולט `TransferTranscript` לכל קריאת סיסמה:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` קושר את התמליל ל-hash של נקודת הכניסה של העסקה להגנה על הפעלה חוזרת.
- `authority_digest` הוא ה-hash של המארח על פני נתוני חותמים/מניין ממוינים; הגאדג'ט בודק שוויון אך אינו חוזר על אימות החתימה. באופן קונקרטי המארח Norito מקודד את ה-`AccountId` (שכבר מטמיע את בקר ה-multisig הקנוני) ומערבב את `b"iroha:fastpq:v1:authority|" || encoded_account` עם Blake2b-256, ומאחסן את ה-`Hash` שנוצר.
- `poseidon_preimage_digest` = Poseidon(account_from || account_to || asset || amount || batch_hash); מבטיח שהגאדג'ט מחשב מחדש את אותו תקציר כמו המארח. בתים הקדם-תמונה בנויים כ-`norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` באמצעות קידוד Norito חשוף לפני שהם מעבירים אותם דרך העוזר המשותף של Poseidon2. תקציר זה קיים עבור תמלול דלתא בודד והושמט עבור קבוצות מרובות דלתא.

כל השדות מסודרים באמצעות Norito כך שהערבויות הדטרמיניזם הקיימות מתקיימות.
גם `from_path` וגם `to_path` נפלטים ככתמי Norito באמצעות
סכימת `TransferMerkleProofV1`: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
גרסאות עתידיות יכולות להרחיב את הסכימה בזמן שהמוכיח אוכף את תג הגרסה
לפני הפענוח. מטא נתונים `TransitionBatch` מטמיעים את התמליל המקודד Norito
וקטור מתחת למפתח `transfer_transcripts` כדי שהמוכיח יוכל לפענח את העד
מבלי לבצע שאילתות מחוץ לפס. כניסות ציבוריות (`dsid`, `slot`, שורשים,
`perm_root`, `tx_set_hash`) נישאים ב-`FastpqTransitionBatch.public_inputs`,
השארת מטא נתונים עבור ניהול חשבונות hash/תמלילים. עד אינסטלציה מארח
אדמות, המוכיח גוזר הוכחות סינתטיות מצמדי המפתח/איזון כך שורות
כלול תמיד נתיב SMT דטרמיניסטי גם כאשר התמליל משמיט את השדות האופציונליים.

## פריסת גאדג'ט

1. **בלוק אריתמטי מאוזן**
   - כניסות: `from_balance_before`, `amount`, `to_balance_before`.
   - צ'קים:
     - `from_balance_before >= amount` (גאדג'ט טווח עם פירוק RNS משותף).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - ארוז בשער מותאם אישית כך שכל שלוש המשוואות צורכות קבוצת שורה אחת.2. **בלום מחויבות פוסידון**
   - מחשב מחדש את `poseidon_preimage_digest` באמצעות טבלת החיפוש המשותפת של Poseidon שכבר נעשה בה שימוש בגאדג'טים אחרים. אין סיבובי פוסידון לכל העברה.

3. **Merkle Path Block**
   - מרחיב את הגאדג'ט הקיים של Kaigi SMT עם מצב "עדכון מזווג". שני עלים (שולח, מקלט) חולקים את אותה עמודה עבור גיבוב אחים, ומפחיתים שורות כפולות.

4. **בדיקת תקציר הרשות**
   - אילוץ שוויון פשוט בין התמצית המסופקת על ידי המארח לבין ערך העד. החתימות נשארות בגאדג'ט הייעודי שלהם.

5. **לולאה אצווה**
   - תוכניות קוראים `transfer_v1_batch_begin()` לפני לולאה של בוני `transfer_asset` ו-`transfer_v1_batch_end()` לאחר מכן. בעוד ה-scope פעיל, המארח מאחסן כל העברה ומפעיל אותם מחדש כ-`TransferAssetBatch` יחיד, תוך שימוש חוזר בהקשר של Poseidon/SMT פעם אחת בכל אצווה. כל דלתא נוספת מוסיפה רק את החשבון ושני המחאות עלים. מפענח התמלול מקבל כעת קבוצות מרובות דלתא ומציג אותן כ-`TransferGadgetInput::deltas` כך שהמתכנן יכול לקפל עדים מבלי לקרוא מחדש את Norito. חוזים שכבר יש להם מטען Norito בהישג יד (למשל, CLI/SDKs) יכולים לדלג על ההיקף לחלוטין על ידי קריאה ל-`transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`, אשר מוסרת למארח אצווה מקודדת במלואה ב-syscall אחת.

# שינויים במארחים ומספקים| שכבה | שינויים |
|-------|--------|
| `ivm::syscalls` | הוסף `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) כך שתוכניות יוכלו לכלול מספר שיחות `transfer_v1` מבלי לפלוט Kotodama ביניים, פלוס Kotodama. (`0x2B`) עבור אצוות מקודדות מראש. |
| `ivm::host` ובדיקות | מארחי ליבה/ברירת מחדל מתייחסים ל-`transfer_v1` כתוספת אצווה בזמן שה-scope פעיל, משטחים `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}`, ומארח ה-WSV המדומה מאחסן ערכים לפני ביצוע כך שמבחני רגרסיה יכולים לקבוע איזון דטרמיניסטי עדכונים.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | פלט `TransferTranscript` לאחר המעבר למצב, בנה רשומות `FastpqTransitionBatch` עם `public_inputs` מפורש במהלך `StateBlock::capture_exec_witness`, והפעל את נתיב ההוכחה של FASTPQ כך שגם Torii/CLI6 הגיבוי הכלי Canontage כניסות `TransitionBatch`. `TransferAssetBatch` מקבץ העברות רציפות לתמלול יחיד, תוך השמטת תקציר ה-poseidon עבור אצווה מרובה-דלתא, כך שהגאדג'ט יוכל לחזור על ערכים באופן דטרמיניסטי. |
| `fastpq_prover` | `gadgets::transfer` מאמת כעת תמלילים מרובי-דלתא (חשבון איזון + תקציר פוסידון) ומציג עדים מובנים (כולל כתמי SMT מותאמים עם מצייני מיקום) עבור המתכנן (`crates/fastpq_prover/src/gadgets/transfer.rs`). `trace::build_trace` מפענח את התמלילים מתוך מטא-נתונים של אצווה, דוחה אצות העברה חסרות מטען `transfer_transcripts`, מצרף את העדים המאושרים ל-`Trace::transfer_witnesses`, ו-`TracePolynomialData::transfer_plan()` שומרת על כל התוכנית המתוכננת. (`crates/fastpq_prover/src/trace.rs`). רתמת הרגרסיה של ספירת השורות נשלחת כעת דרך `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), ומכסה תרחישים של עד 65536 שורות מרופדות, בעוד שחיווט ה-SMT המזווג נשאר מאחורי אבן הדרך TF-3 של עוזר האצווה (מחזיקי מיקום מחזיקים בנחיתות של פריסת העקבות הזו). |
| Kotodama | מוריד את העזר `transfer_batch((from,to,asset,amount), …)` ל-`transfer_v1_batch_begin`, שיחות `transfer_asset` רציפות ו-`transfer_v1_batch_end`. כל ארגומנט tuple חייב לעקוב אחר הצורה `(AccountId, AccountId, AssetDefinitionId, int)`; העברות בודדות שומרות על הקבלן הקיים. |

דוגמה לשימוש ב-Kotodama:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` מבצע את אותן בדיקות הרשאה וחשבון כמו קריאות `Transfer::asset_numeric` בודדות, אך מתעד את כל הדלתות בתוך `TransferTranscript` יחיד. תמלילים מרובי דלתא שולפים את עיכול הפוסידון עד שההתחייבויות לכל דלתא נוחתות במעקב. בונה Kotodama פולט כעת את מערכות ההתחלה/הסיום באופן אוטומטי, כך שחוזים יכולים לפרוס העברות אצווה ללא קידוד ידני של עומסי Norito.

## רתמת רגרסיה של ספירת שורות

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) מסנתז אצווה מעבר של FASTPQ עם ספירות בורר הניתנות להגדרה ומדווח על סיכום `row_usage` המתקבל (`total_rows`, אורך לכל בורר סופר ספירת רישום/יחס). לכוד אמות מידה עבור תקרה 65536 שורות עם:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```ה-JSON הנפלט משקף את חפצי האצווה של FASTPQ ש-`iroha_cli audit witness` פולט כעת כברירת מחדל (עבר את `--no-fastpq-batches` כדי לדכא אותם), כך ש-`scripts/fastpq/check_row_usage.py` ושער ה-CI יכולים להבדיל בין הריצות הסינתטיות לשינויים תקפים של תכניות תוכנית תקפות.

# תוכנית השקה

1. **TF-1 (תמלול אינסטלציה)**: ✅ `StateTransaction::record_transfer_transcripts` פולט כעת תמלילים Norito עבור כל `TransferAsset`/אצווה, `sumeragi::witness::record_fastpq_transcript` מאחסן אותם בתוך העד הגלובלי, ו-I08NI00X build `fastpq_batches` עם `public_inputs` מפורש עבור מפעילים ונתיב ההוכחה (השתמש ב-`--no-fastpq-batches` אם אתה צריך רזה יותר פלט).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness .rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (יישום גאדג'ט)**: ✅ `gadgets::transfer` מאמת כעת תמלילים מרובי-דלתא (חשבון איזון + תקציר פוסידון), מסנתז הוכחות SMT מזווגות כאשר מארחים משמיטים אותן, חושף עדים מובנים באמצעות Norito000,0010,010,010,001 משחיל את העדים האלה לתוך `Trace::transfer_witnesses` תוך אכלוס עמודות SMT מההוכחות. `fastpq_row_bench` לוכד את רתמת הרגרסיה של 65536 שורות כך שמתכננים עוקבים אחר השימוש בשורות מבלי להפעיל מחדש את Norito מטענים.【ארגזים/fastpq_prover/src/gadgets/transfer.rs:1】【ארגזים/fastpq_prover/src/trace.rs:1】【ארגזים/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (עזר אצווה)**: אפשר את ה-Batch syscall + Kotodama בונה, כולל יישום רציף ברמת המארח ולולאת גאדג'ט.
4. **TF-4 (טלמטריה ומסמכים)**: עדכון `fastpq_plan.md`, `fastpq_migration_guide.md` וסכימות לוח המחוונים להקצאת פני שטח של שורות העברה לעומת גאדג'טים אחרים.

# שאלות פתוחות

- **מגבלות תחום**: מתכנן ה-FFT הנוכחי נכנס לפאניקה בעקבות עקבות מעבר ל-2¹⁴ שורות. TF-2 צריך להגדיל את גודל הדומיין או לתעד יעד מופחת מופחת.
- **אצות ריבוי נכסים**: הגאדג'ט הראשוני מניח את אותו מזהה נכס לכל דלתא. אם אנו זקוקים לקבוצות הטרוגניות, עלינו להבטיח שהעד של פוסידון יכלול את הנכס בכל פעם כדי למנוע הצצה חוזרת של נכסים.
- **שימוש חוזר ב-Autority Digest**: לטווח ארוך נוכל לעשות שימוש חוזר באותו תקציר עבור פעולות מורשות אחרות כדי להימנע מחישוב מחדש של רשימות החותמים לכל קריאת מערכת.


מסמך זה עוקב אחר החלטות עיצוב; שמור אותו מסונכרן עם ערכי מפת הדרכים כאשר אבני דרך נוחתות.