<!-- Hebrew translation of docs/source/light_client_da.md -->

---
lang: he
direction: rtl
source: docs/source/light_client_da.md
status: complete
translator: manual
---

<div dir="rtl">

# דגימת זמינות נתונים ללייט-קליינטים

API דגימת לייט-קליינט מאפשר למפעילים מאומתים לקבל דגימות של מקטעי RBC עם הוכחת Merkle עבור בלוק שבשלבי עיבוד. לקוחות קלים יכולים לשגר בקשות דגימה אקראיות, לוודא את ההוכחות מול שורש המקטעים שפורסם, ולהגביר ביטחון בזמינות המידע – וכל זאת בלי למשוך את כולה.

## נקודת קצה

```
POST /v1/sumeragi/rbc/sample
```

הקריאה דורשת כותרת `X-API-Token` התואמת לאחד מטוקני ה-Torii שהוגדרו. נוסף לכך חלים מנגנוני הגבלת קצב ותקציב יומי של בתים לכל מזמין; חריגה מאחד מהם מחזירה HTTP 429.

### גוף הבקשה

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – גיבוב היעד (hex).
* `height`,‏ `view` – זוג מזהה עבור סשן ה-RBC.
* `count` – מספר הדגימות המבוקש (ברירת מחדל 1, מוגבל בקונפיגורציה).
* `seed` – זרע RNG אופציונלי לדגימה דטרמיניסטית לשחזור.

### גוף התגובה

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

כל דגימה מכילה את אינדקס המקטע, נתוני המטען ב-hex, דיגסט SHA-256 של העלה והוכחת הכללה ב-Merkle (אחים אופציונליים מקודדים כ-hex). הלקוח יכול לאמת את ההוכחות בעזרת `chunk_root`.

## מגבלות ותקציבים

* **מספר דגימות מקסימלי לבקשה** – מוגדר ב-`torii.rbc_sampling.max_samples_per_request`.
* **כמות בתים מקסימלית לבקשה** – נאכפת ב-`torii.rbc_sampling.max_bytes_per_request`.
* **תקציב בתים יומי** – נמדד לכל מזמין דרך `torii.rbc_sampling.daily_byte_budget`.
* **Rate limiting** – נאכף באמצעות דלי טוקנים ייעודי (`torii.rbc_sampling.rate_per_minute`).

בקשות שעוברות את אחד הספים יחזירו HTTP 429 (CapacityLimit). אם חנות המקטעים אינה זמינה או שהסשן חסר נתוני מטען, המערכת תחזיר HTTP 404.

## אינטגרציית SDK

### JavaScript

חבילת `@iroha/iroha-js` כוללת את ההילפר `ToriiClient.sampleRbcChunks`, כך שניתן להזמין דגימות מאומתות בלי לכתוב שכבת HTTP ייעודית. ההילפר מאמת שכל השדות המקודדים ב-hex תקינים ומחזיר אובייקטים טיפוסיים שתואמים לסכמת התגובה.

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("סשן ה-RBC עדיין לא זמין");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

אם השרת מחזיר מטען שגוי, ההילפר זורק חריגה וכך נשמרת אותה מדיניות פריטי-פריטי כמו ב-Rust (`iroha_client::ToriiClient::sample_rbc_chunks`) וב-Python (`IrohaToriiClient.sample_rbc_chunks`). בחרו את ה-SDK המתאים לצינור הדגימה שלכם.

</div>
