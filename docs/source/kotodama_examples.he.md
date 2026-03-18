<!-- Hebrew translation of docs/source/kotodama_examples.md -->

---
lang: he
direction: rtl
source: docs/source/kotodama_examples.md
status: complete
translator: manual
---

<div dir="rtl">

# סקירת דוגמאות Kotodama

עמוד זה מציג דוגמאות Kotodama תמציתיות והקשר שלהן לקריאות מערכת של IVM וארגומנטים במוסכמת ה-pointer‑ABI. ראו גם:
- מקורות הניתנים להרצה: `examples/`
- טבלת ה-ABI הקנונית: `docs/source/ivm_syscalls.md`
- המפרט המלא של השפה: `kotodama_grammar.md`

## Hello + Account Detail

מקור: `examples/hello/hello.ko`

```kotodama
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

מיפוי (pointer‑ABI):
- `authority()` → ‏`SCALL 0xA4` (ההוסט כותב `&AccountId` ל-`r10`)
- `set_account_detail(a, k, v)` → העברת `r10=&AccountId`, ‏`r11=&Name`, ‏`r12=&Json`, ואז `SCALL 0x1A`

## Asset Transfer

מקור: `examples/transfer/transfer.ko`

```kotodama
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
      account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```

מיפוי (pointer‑ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, ‏`r11=&AccountId(to)`, ‏`r12=&AssetDefinitionId(def)`, ‏`r13=amount`, ולאחר מכן `SCALL 0x24`

## יצירה והעברה של NFT

מקור: `examples/nft/nft.ko`

```kotodama
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let recipient = account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

מיפוי (pointer‑ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, ‏`r11=&AccountId(owner)`, ‏`SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, ‏`r11=&NftId`, ‏`r12=&AccountId(to)`, ‏`SCALL 0x26`

## איטרציית Map דטרמיניסטית (תכנון)

לולאת for על Map מחייבת גבול. עבור איטרציה מרובת־פריטים נדרש Map במצב (state); הקומפיילר מקבל `.take(n)` או אורך מרבי שהוצהר מראש.

```kotodama
// דוגמת תכנון (איטרציה מחייבת גבול ושמירה ב־state)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

סמנטיקה:
- קבוצת האיטרציה היא צילום מצב בעת הכניסה ללולאה; הסדר הוא לקסיקוגרפי לפי בתים של Norito עבור המפתח.
- שינוי מבני ב־`M` בתוך הלולאה גורר מלכודת `E_ITER_MUTATION`.
- ללא גבול הקומפיילר מפיק `E_UNBOUNDED_ITERATION`.

## עדכוני Frame מקוטעים עם opcode רחב

הבילט-אינים הרחבים של Kotodama מכוונים לפריסת אופרנדים על 8 ביט בקידוד הרחב של IVM. טעינות/אחסונים של 128 ביט משתמשים באופרנד השלישי עבור הרגיסטר הגבוה, ולכן הרגיסטר הבסיסי חייב להכיל כבר את הכתובת הסופית. יש לבצע התאמת בסיס באמצעות `ADDI` לפני הקריאה/כתיבה:

```rust
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```

עדכוני פריים מקוטעים מקדמים את הבסיס בצעדים של 16 בתים כך שזוג הרגיסטרים ש-`STORE128` כותב נשאר מיושר. אותה תבנית חלה על `LOAD128`; ביצוע `ADDI` עם הצעד הרצוי לפני כל טעינה משאיר את הרגיסטר הגבוה צמוד לאופרנד השלישי. כתובות לא מיושרות לוכדות עם `VMError::MisalignedAccess`, בהתאם לבדיקות ב-`crates/ivm/tests/wide_memory128.rs`.

תכניות שמפיקות את עזרי ה-128 ביט חייבות להצהיר על יכולת וקטורית. קומפיילר Kotodama מפעיל אוטומטית את ביט `VECTOR` כאשר מזוהות הוראות `LOAD128`/`STORE128`. הרצתן בלי הביט תוביל ל-`VMError::VectorExtensionDisabled`.

## הורדת תנאי רחב

בעת הורדת `if`/`else` (או תנאי משולש) לקוד רחב, Kotodama מייצרת רצף קבוע: `BNE cond, zero, +2` ולאחריו שני `JAL`.

1. ה-`BNE` הקצר משאיר את ההסתעפות בטווח 8 ביט בכך שהוא מדלג מעל `JAL` של ה-fallthrough.
2. ה-`JAL` הראשון פונה לבלוק ה-`else` (כאשר התנאי שקרי).
3. ה-`JAL` השני קופץ לבלוק ה-`then` (כאשר התנאי אמת).

התבנית מבטיחה שהקיזוז של הבדיקה נשאר תמיד בתוך ±127 מילים, בעוד שגופי `then`/`else` יכולים להתרחב כרצונם בעזרת `JAL` רחב. בדיקת הרגרסיה `crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` מקבעת את הרצף.

### דוגמת הורדה

```kotodama
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

הקומפילציה מניבה שלד הוראות רחב (מספרי רגיסטרים ואופסטים מוחלטים תלויים בהקשר):

```
BNE cond_reg, x0, +2    # דלג על קפיצת ה-fallthrough כשהתנאי אמת
JAL x0, else_offset     # מבוצע כשהתנאי שקר
JAL x0, then_offset     # מבוצע כשהתנאי אמת
```

הוראות בהמשך מייצרות את הקבועים וקובעות את ערך ההחזרה. מאחר שה-`BNE` מדלג מעל ה-`JAL` הראשון, הקיזוז המותנה נשאר תמיד `+2` מילים גם אם גופי הבלוקים גדלים.

</div>
