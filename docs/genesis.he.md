<!-- Hebrew translation of docs/genesis.md -->

---
lang: he
direction: rtl
source: docs/genesis.md
status: complete
translator: manual
---

<div dir="rtl">

# תצורת Genesis

קובץ `genesis.json` מגדיר את הטרנזקציות הראשונות שרצות בעת עליית רשת Iroha. זהו אובייקט JSON עם השדות הבאים:

- `chain` – מזהה שרשרת ייחודי.
- `executor` (אופציונלי) – נתיב ל-bytecode של ה-Executor (`.to`). אם קיים, Genesis יכלול הוראת Upgrade כטרנזקציה הראשונה. אם חסר, לא מתבצע שדרוג וה-Executor המובנה מופעל.
- `ivm_dir` – ספרייה המכילה ספריות bytecode של IVM. ברירת המחדל `"."`.
- `consensus_mode` – מצב הקונצנזוס שמפורסם במניפסט. חובה; יש להשתמש ב-`"Npos"` ל‑Iroha3 (ברירת מחדל) וב‑`"Permissioned"` ל‑Iroha2.
- `transactions` – רשימת טרנזקציות Genesis המבצעות ברצף. כל טרנזקציה יכולה להכיל:
  - `parameters` – פרמטרים התחלתיים לרשת.
  - `instructions` – הוראות מקודדות Norito.
  - `ivm_triggers` – טריגרים עם קבצי bytecode.
  - `topology` – טופולוגיית Peer ראשונית. כל רשומה משתמשת ב-`peer` (PeerId כמחרוזת, כלומר המפתח הציבורי) וב-`pop_hex`; אפשר להשמיט `pop_hex` בזמן ההכנה, אך הוא חייב להיות קיים לפני חתימה.
- `crypto` – צילום מצב של הגדרות ההצפנה מתוך `iroha_config.crypto` (`default_hash`, `allowed_signing`, `allowed_curve_ids`, `sm2_distid_default`, `sm_openssl_preview`). `allowed_curve_ids` משקף את `crypto.curves.allowed_curve_ids` ומפרסם אילו מזהי עקומות קונטרולריים מאושרים ברשת. אם מוסיפים `sm2` לרשימת החתימות חובה לעדכן גם את ערך ה-hash ל-`sm3-256`; בנייה ללא התמיכה ב-`sm` תדחה הגדרות `sm2` לחלוטין.

דוגמה (פלט מקוצר של `kagami genesis generate default --consensus-mode npos`):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

## הכנת בלוק `crypto` לרשתות SM2/SM3

הדרך המהירה היא להריץ את xtask כדי להפיק במכה אחת את מאגר המפתחות וקטע התצורה המוכן להדבקה:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` יכיל כעת:

```toml
# חומר מפתח לחשבון
public_key = "sm2:86264104..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # להסרה כאשר נשארים במצב אימות בלבד
allowed_curve_ids = [1]               # הוסיפו מזהים נוספים (לדוגמה 15 עבור SM2) כאשר פותחים קונטרולרים חדשים
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # אופציונלי: רק כאשר מפעילים את ספק OpenSSL/Tongsuo
```

העתיקו את ערכי `public_key`/`private_key` להגדרות החשבון או הלקוח, ועדכנו את בלוק `crypto` של `genesis.json` כך שיתאים לתצורה מהקובץ (למשל קבעו `default_hash` ל-`sm3-256`, הוסיפו `"sm2"` ל-`allowed_signing`, ועדכנו את `allowed_curve_ids`). Kagami תסרב מניפסטים שבהם אלגוריתם ההאש/העקומה והרשימות אינם מסונכרנים.

> **טיפ:** אפשר להזרים את הקטע ישירות ל-stdout בעזרת `--snippet-out -`. באותו אופן ניתן להשתמש ב-`--json-out -` כדי להציג את מאגר המפתחות על המסך במקום לכתוב לקובץ.

אם מעדיפים להפעיל את פקודות ה-CLI ברמת הנמוכה, השתמשו ברצף הבא:

```bash
# 1. חומר מפתח דטרמיניסטי (מפיק JSON לדיסק)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. בניית קטע התצורה להדבקה בקבצי לקוח/קונפיגורציה
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **טיפ:** הפקודות מסתמכות על `jq` כדי לחסוך העתקה ידנית. אם הכלי לא מותקן, פתחו את `sm2-key.json`, העתיקו את השדה `private_key_hex`, והעבירו אותו ישירות אל `crypto sm2 export`.

> **מדריך הגירה:** לשדרוג רשת קיימת לתצורת SM2/SM3/SM4, עיינו ב-
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md),
> שמפרט שכבות `iroha_config`, יצירת מניפסטים עם `kagami --enable-sm` ותכנית נסיגה.

## יצירה ואימות

1. יצירת תבנית:
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
  ```
   הפרמטר `--consensus-mode` קובע אילו פרמטרי קונצנזוס Kagami זורע בתוך בלוק ה-`parameters`. ב‑Iroha3 `npos` הוא חובה ואין תמיכה בהחלפה מדורגת; ב‑Iroha2 ברירת המחדל היא `permissioned`, וניתן לבצע מעבר מדורג באמצעות `--next-consensus-mode`/`--mode-activation-height`. בחירה ב‑`npos` תזרע את המטען `sumeragi_npos_parameters`, ובמהלך נירמול/חתימה הוא מתורגם להוראות `SetParameter` בבלוק.
2. עריכת `genesis.json` (אופציונלי), ולאחר מכן אימות וחתימה:
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   להפעלת אלגוריתמי SM2/SM3/SM4 כבר ב-Genesis הגדירו `--default-hash sm3-256` והוסיפו `--allowed-signing sm2` (ניתן לחזור על הדגל עבור אלגוריתמים נוספים). במידת הצורך אפשר לעדכן מזהה מובחן באמצעות `--sm2-distid-default <ID>`.

   כאשר מפעילים את `irohad` עם `--genesis-manifest-json` בלבד (ללא בלוק Genesis חתום), תצורת ההצפנה של הרצה נטענת אוטומטית מתוך מקטע `crypto` שבמניפסט. אם מספקים גם בלוק Genesis חתום, עדיין נדרש שהתצורה והמניפסט יתאימו במדויק.

הפקודה `kagami genesis sign` מאמתת את ה-JSON ומפיקה בלוק Norito לשימוש עם `genesis.file` בקובץ ההגדרות של הצומת. קובץ `genesis.signed.nrt` נמצא כבר בפורמט Wire קנוני (בית גרסה ולאחריו כותרת Norito). הפיצו תמיד את הקובץ הממוסגר, והעדיפו את הסיומת `.nrt`. אם אינכם צריכים לשדרג את ה-Executor ב-Genesis, ניתן להשמיט את `executor`.

בעת חתימה על מניפסטים של NPoS (`--consensus-mode npos` או החלפה מדורגת ב‑Iroha2 בלבד), `kagami genesis sign` דורש את המטען `sumeragi_npos_parameters`; צרו אותו עם `kagami genesis generate --consensus-mode npos` או הוסיפו את הפרמטר ידנית.
כברירת מחדל, `kagami genesis sign` משתמשת ב-`consensus_mode` מהמניפסט; אפשר להעביר `--consensus-mode` כדי לעקוף.

## מה אפשר לעשות ב-Genesis

Genesis תומך בפעולות הבאות; Kagami מרכיבה אותן בסדר דטרמיניסטי:

- **פרמטרים**: הגדרת ערכים התחלתיים ל-Sumeragi, Block, Transaction, Executor, חוזים חכמים ופרמטרים מותאמים. Kagami זורעת את `Sumeragi::NextMode` ו-`sumeragi_npos_parameters` בתוך בלוק ה-`parameters`, והבלוק החתום מכיל את הוראות ה-`SetParameter` שנוצרו.
- **הוראות מובנות**: רישום/הסרה של דומיין, חשבון, הגדרת נכס; יציקת/שריפת/העברת נכסים; העברת בעלות; עריכת Metadata; הענקת הרשאות ותפקידים.
- **טריגרי IVM**: רישום טריגרים המפעילים bytecode (נתיבים יחסיים ל-`ivm_dir`).
- **טופולוגיה**: הגדרת כלי-peer ראשוניים דרך `topology` עם רשומות
  `{ "peer": "<public_key>", "pop_hex": "<hex>" }`; ניתן להשמיט `pop_hex`
  בזמן ההכנה, אך הוא חייב להיות קיים לפני חתימה.
- **שדרוג Executor (אופציונלי)**: אם `executor` קיים, Genesis מוסיף טרנזקציית Upgrade אחת בתחילה.

### סדר טרנזקציות

1. (אופציונלי) שדרוג Executor.
2. עבור כל טרנזקציה:
   - עדכוני פרמטר.
   - הוראות מובנות.
   - רישום טריגרים.
   - טופולוגיה.

גם Kagami וגם הקוד בצומת מוודאים שהסדר נשמר.

## תהליך מומלץ

- התחילו מתבנית עם Kagami:
  - ISI בלבד: `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (ברירת מחדל ב‑Iroha3; השתמשו ב‑`--consensus-mode permissioned` ל‑Iroha2)
  - עם שדרוג Executor: הוספת `--executor <path/to/executor.to>`
- `<PK>` יכול להיות כל multihash הנתמך ע"י `iroha_crypto::Algorithm`, כולל GOST כשבונים עם `--features gost`.
- אימות בזמן עריכה: `kagami genesis validate genesis.json`
- חתימה לפריסה: `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- קביעת הצמתים: `genesis.file = genesis.signed.nrt`, ‎`genesis.public_key = <PK>`‎.

הערות:
- התבנית "default" של Kagami רושמת דומיין, חשבונות ונכסים לדוגמה עם ISI בלבד.
- אם מוסיפים שדרוג Executor, עליו להיות טרנזקציה ראשונה; Kagami אוכפת זאת.
- `kagami genesis validate` מזהה שמות לא תקפים והוראות פגומות לפני חתימה.

## הרצה עם Docker/Swarm

כלי Docker Compose / Swarm במאגר תומכים בשני מצבים:

- ללא Executor: הפקודה מוחקת שדה `executor` ריק וחותמת.
- עם Executor: הנתיב יחסי מתורגם לנתיב בתוך הקונטיינר ונחתם.

כך ניתן לפתח גם על מכונות בלי דוגמאות IVM מוכנות, ועדיין לתמוך בשדרוגי Executor בעת הצורך.

</div>
