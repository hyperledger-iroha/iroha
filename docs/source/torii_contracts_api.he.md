<!-- Hebrew translation of docs/source/torii_contracts_api.md -->

---
lang: he
direction: rtl
source: docs/source/torii_contracts_api.md
status: complete
translator: manual
---

<div dir="rtl">

# Torii Contracts API (מניפסטים ופריסה)

מסמך זה מתאר את נקודות הקצה HTTP שנועדו לפרסום ואחזור מניפסטים של חוזים חכמים. הן מעטפת דקה מעל טרנזקציות וקריאות קריאה-בלבד על השרשרת; הסמנטיקה הקונצנזואלית נשארת על השרשרת.

## נקודות קצה

- **POST `/v1/contracts/code`** – עוטף `RegisterSmartContractCode` בטרנזקציה חתומה ושולח אותה.
  - גוף: `RegisterContractCodeDto` (JSON או Norito JSON; ראו סכימה בהמשך).
  - תשובה: `202 Accepted` עם קבלה; שגיאות קבלה אחרות בהתאם.
  - הרשאה: נדרש `CanRegisterSmartContractCode`.
  - טלמטריה: שגיאות מעבד מוסיפות ל-`torii_contract_errors_total{endpoint="code"}`, הפעלת ה-rate limiter מוסיפה ל-`torii_contract_throttled_total{endpoint="code"}`.
  - תרחישים עיקריים:

    | תרחיש | סטטוס | גוף | הערות |
    | --- | --- | --- | --- |
    | התקבל לתור | `202 Accepted` | גוף ריק | מזהה טרנזקציה מהחתימה; יש לנטר ב-pipeline. |
    | חסר הרשאה | `202 Accepted` | ראה דוגמה | התור מקבל, מאוחר יותר נרשם `ValidationFail::NotPermitted`. |
    | `manifest.code_hash`/`abi_hash` אינם hex חוקי | `400 Bad Request` | הודעת `invalid JSON body` | בדיקה בשכבת Norito JSON. |
    | התור מלא | `429 Too Many Requests` | JSON מובנה (`code`, `message`, `queue`, `retry_after_seconds`) | כולל `Retry-After` והידוק עומס. |

    דוגמה לסטטוס שנרשם ב-`/v1/pipeline/transactions/status` במקרה הרשאה חסרה:

    ```json
    {
      "kind": "Transaction",
      "content": {
        "hash": "…",
        "status": {
          "kind": "Rejected",
          "content": "<base64 TransactionRejectionReason>"
        }
      }
    }
    ```

    דוגמת גוף שגיאת תור:

    ```json
    {
      "code": "queue_full",
      "message": "transaction queue is at capacity",
      "queue": {
        "state": "saturated",
        "queued": 65536,
        "capacity": 65536,
        "saturated": true
      },
      "retry_after_seconds": 1
    }
    ```

- **GET `/v1/contracts/code/{code_hash}`** – מחזיר `ContractManifest` לפי `code_hash`.
- **POST `/v1/contracts/deploy`** – מקבל bytecode Base64, מחשב `code_hash`/`abi_hash`, ושולח `RegisterSmartContractCode` + `RegisterSmartContractBytes` בטרנזקציה אחת.
  - גוף: `DeployContractDto`; תשובה: `DeployContractResponseDto`.
  - גודל הקוד מוכתב ע"י הפרמטר המותאם `max_contract_code_bytes` (ברירת מחדל 16 MiB). יש לעדכן את הערך לפני העלאת תוכניות גדולות יותר.
  - טלמטריה: שגיאות → `torii_contract_errors_total{endpoint="deploy"}`; הפעלת limiter → `torii_contract_throttled_total{endpoint="deploy"}`.
- **POST `/v1/contracts/instance`** – מקבל bytecode Base64 ויעד `(namespace, contract_id)`, ומבצע `RegisterSmartContractCode` + `RegisterSmartContractBytes` + `ActivateContractInstance` באותה טרנזקציה.
  - תשובה: `{ ok, namespace, contract_id, code_hash_hex, abi_hash_hex }`.
  - טלמטריה: שגיאות → `torii_contract_errors_total{endpoint="instance"}`; הפעלת limiter → `torii_contract_throttled_total{endpoint="instance"}`.
- **GET `/v1/contracts/code-bytes/{code_hash}`** – מחזיר `{ code_b64 }`.
- **POST `/v1/contracts/instance/activate`** – שולח `ActivateContractInstance` עבור `(namespace, contract_id)`.
  - גוף: `ActivateInstanceDto`; תשובה: `ActivateInstanceResponseDto`.
  - טלמטריה: שגיאות → `torii_contract_errors_total{endpoint="activate"}`; הפעלת limiter → `torii_contract_throttled_total{endpoint="activate"}`.
- **GET `/v1/contracts/instances/{ns}`** – מציג מופעי חוזים פעילים ב-`ns` עם פרמטרי סינון/מיון זהים לאנדפוינט הגוורננס.

## סכימות

### RegisterContractCodeDto

```jsonc
{
  "authority": "alice@wonderland",
  "private_key": "ed25519:...",
  "manifest": {
    "code_hash": "0123…cdef",
    "abi_hash": "89ab…7654",
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0
  }
}
```

- `manifest.code_hash` משמש כמפתח האחסון אם סופק.
- `manifest.abi_hash` נבדק מול מדיניות ה-ABI של הנוד.

### DeployContractDto

בקשה להעלאת bytecode קומפילט ולקבלת מניפסט וחישובים מהנוד.

```jsonc
{
  "authority":   "alice@wonderland", // מזהה חשבון (מחרוזת)
  "private_key": "ed25519:0123…",    // ExposedPrivateKey (hex רגיל או עם קידומת אלגוריתם)
  "code_b64":    "Base64Payload=="
}
```

- `code_b64` חייב להתפרש כ-header תקין של IVM עם `abi_version == 1`.
- המניפסט נבנה בצד השרת; הלקוח אינו שולח אחד בבקשה זו.
- פענוח ל-bytecode גדול מהערך `max_contract_code_bytes` יוביל ל-`InvariantViolation` עם ההודעה `code bytes exceed cap`.

### DeployContractResponseDto

```jsonc
{
  "ok": true,
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex":  "89ab…7654"
}
```

### קידודי JSON

- `Hash` → Hex באורך 64 תווים (32 בתים, אותיות קטנות).
- `AccountId` → מחרוזת בפורמט `<name>@<domain>`.
- `ExposedPrivateKey` מקבל גם hex רגיל וגם גרסה עם קידומת אלגוריתם (למשל `ed25519:…`). תשובות חוזרות כ-hex רגיל.

### תשובת GET

```jsonc
{
  "manifest": { … }
}
```

### DeployAndActivateInstanceDto

```jsonc
{
  "authority": "alice@wonderland",
  "private_key": "ed25519:…",
  "namespace": "apps",
  "contract_id": "calc.v1",
  "code_b64": "…",
  "manifest": {
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0,
    "access_set_hints": {
      "read_keys": ["account:alice@wonderland"],
      "write_keys": ["asset:usd#wonderland"]
    }
  }
}
```

- אם `manifest.code_hash` נשלח, הוא חייב להתאים ל-hash שמחושב מה-bytecode (אחרת הבקשה תידחה).
- ה-node מחשב מחדש `abi_hash` לפי ה-IVM header (נכון לעכשיו ABI v1) ומוודא שכל ערך שסופק תואם.
- שדות נוספים (fingerprint/bitmap/access hints) נשמרים כפי שהם במניפסט.

### DeployAndActivateInstanceResponseDto

```jsonc
{
  "ok": true,
  "namespace": "apps",
  "contract_id": "calc.v1",
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex": "89ab…7654"
}
```

### ActivateInstanceDto

בקשה לקשור `code_hash` קיים למזהה חוזה ב-namespace נתון.

```jsonc
{
  "authority":   "alice@wonderland",
  "private_key": "ed25519:0123…",
  "namespace":   "apps",
  "contract_id": "calc.v1",
  "code_hash":   "89ab…7654"
}
```

- `code_hash` חייב להיות Hex באורך 64 (32 בתים). קידומת `0x` תוסר אם קיימת.
- המניפסט וה-bytecode עבור ה-`code_hash` חייבים להיות קיימים בשרשרת; אחרת הטרנזקציה תידחה.
- namespaces מוגנים עדיין דורשים מטא-דאטה `gov_namespace`/`gov_contract_id` בהמשך הצינור.

### ActivateInstanceResponseDto

```jsonc
{
  "ok": true
}
```

### מטעני Norito

כל ה-DTO-ים לעיל מממשים גם `JsonSerialize` וגם `NoritoSerialize`. ניתן לשלוח JSON רגיל או Norito JSON. בעת יצירת מטען Norito בקוטודמה או בבדיקות, מומלץ להשתמש ב-`norito::json::json!` עם אותם שמות שדות וקידודים כדי ש-`NoritoJson<T>` יפענח באופן דטרמיניסטי.

### ריסון קצב וטלמטריה

- `torii.deploy_rate_per_origin_per_sec` ו-`torii.deploy_burst_per_origin` מגדירים את דלי-הטוקנים המשותף לנקודות הקצה `/v1/contracts/{code,deploy,instance,instance/activate}`. ברירת מחדל: 4 בקשות לשנייה עם burst של 8 לכל מקור (`X-API-Token`, כתובת IP, וזוג נקודת קצה).
- בקשות שנחסמות על ידי ה-limiter מעלות את `torii_contract_throttled_total{endpoint}` כש־`endpoint` הוא `code` / `deploy` / `instance` / `activate`.
- כל שגיאה במעבד (גוף לא תקין, חוסר הרשאה, כשל תור) מעלה את `torii_contract_errors_total{endpoint}`. מומלץ לנטר יחד עם מדדי התור.

## דוגמאות

פרסום מניפסט:

```bash
curl -s -X POST ... http://127.0.0.1:8080/v1/contracts/code
```

שליפת מניפסט או קוד בייטס:

```bash
curl -s http://127.0.0.1:8080/v1/contracts/code/<hash>
# …
```

פרסום והפעלת מופע בבקשה אחת:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "alice@wonderland",
        "private_key": "ed25519:…",
        "namespace": "apps",
        "contract_id": "calc.v1",
        "code_b64": "…"
      }' \
  http://127.0.0.1:8080/v1/contracts/instance | jq .
```

הפעלה של מופע קיים (לאחר שה-bytecode וה manifest כבר נשמרו):

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "alice@wonderland",
        "private_key": "ed25519:…",
        "namespace": "apps",
        "contract_id": "calc.v1",
        "code_hash": "<32-byte-hex>"
      }' \
  http://127.0.0.1:8080/v1/contracts/instance/activate | jq .
```

### חישוב `abi_hash`

```bash
iroha ivm abi-hash --policy v1 --uppercase
```

## אבטחה וממשל

- רק בעלי `CanRegisterSmartContractCode` רשאים להגיש מניפסטים; הענקה נשלטת ע"י ממשל.
- קריאות GET הן קריאה-בלבד לפי `code_hash`, אך הנוד רשאי לאכוף מדיניות גישה נוספת.

</div>
