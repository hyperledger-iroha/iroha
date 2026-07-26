---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# התחלה מהירה של Python SDK

Python SDK (`iroha-python`) משקף את עוזרי הלקוח Rust כך שתוכל
לקיים אינטראקציה עם Torii מסקריפטים, מחברות או מקצה עורפי אינטרנט. ההתחלה המהירה הזו
מכסה התקנה, הגשת עסקאות והזרמת אירועים. ליותר עמוק
כיסוי ראה `python/iroha_python/README.md` במאגר.

## 1. התקן

```bash
pip install iroha-python
```

תוספות אופציונליות:

- `pip install aiohttp` אם אתה מתכנן להפעיל את הגרסאות האסינכרוניות של
  עוזרי סטרימינג.
- `pip install pynacl` כאשר אתה צריך גזירת מפתח Ed25519 מחוץ ל-SDK.

## 2. צור לקוח וחותמים

```python
from iroha_python import (
    ToriiClient,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")  # replace with secure storage
authority = pair.default_account_id("wonderland")

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
    auth_token="dev-token",  # optional: omit if Torii does not require a token
    telemetry_url="http://127.0.0.1:8080",  # optional
)
```

`ToriiClient` מקבל ארגומנטים נוספים של מילות מפתח כגון `timeout_ms`,
`max_retries`, ו-`tls_config`. העוזר `resolve_torii_client_config`
מנתח מטען תצורת JSON אם אתה רוצה זוגיות עם Rust CLI.

## 3. שלח עסקה

ה-SDK שולח בוני הוראות ועוזרים לעסקאות, כך שאתה בונה לעתים רחוקות
מטענים Norito ביד:

```python
from iroha_python import Instruction

instruction = Instruction.register_domain("research")

envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,          # poll until the transaction reaches a terminal status
    fetch_events=True,  # include intermediate pipeline events
)

print("Final status:", status)
```

`build_and_submit_transaction` מחזירה גם את המעטפה החתומה וגם את האחרונה
מצב נצפה (לדוגמה, `Committed`, `Rejected`). אם כבר יש לך חתום
במעטפת העסקה השתמש ב-`client.submit_transaction_envelope(envelope)` או ב-
JSON-centric `submit_transaction_json`.

## 4. מצב שאילתה

לכל נקודות הקצה של REST יש עוזרי JSON ורבים חושפים מחלקות נתונים מוקלדות. עבור
לדוגמה, רישום דומיינים:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

עוזרים המודעים לעידון (לדוגמה, `list_accounts_typed`) מחזירים אובייקט
מכיל גם `items` וגם `next_cursor`.

## 5. הזרם אירועים

Torii נקודות קצה SSE נחשפות באמצעות גנרטורים. ה-SDK מתחדש אוטומטית
כאשר `resume=True` ואתה מספק `EventCursor`.

```python
from iroha_python import PipelineEventFilterBox, EventCursor

cursor = EventCursor()

for event in client.stream_pipeline_blocks(
    status="Committed",
    resume=True,
    cursor=cursor,
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

שיטות נוחות אחרות כוללות `stream_pipeline_transactions`,
`stream_events` (עם בוני מסננים מודפסים), ו-`stream_verifying_key_events`.

## 6. השלבים הבאים

- חקור את הדוגמאות תחת `python/iroha_python/src/iroha_python/examples/`
  עבור זרימות מקצה לקצה המכסות ממשל, עוזרי גשר ISO ו-Connect.
- השתמש ב-`create_torii_client` / `resolve_torii_client_config` כאשר אתה רוצה
  אתחול את הלקוח מקובץ או סביבה `iroha_config` JSON.
- עבור Norito RPC או ממשקי API ספציפיים ל-Connect, בדוק את המודולים המיוחדים כגון
  `iroha_python.norito_rpc` ו-`iroha_python.connect`.

עם אבני הבניין האלה אתה יכול לממש את Torii מ-Python מבלי לכתוב
דבק HTTP משלך או Norito codec. ככל שה-SDK מתבגר, רמה גבוהה נוספת
יתווספו בונים; עיין ב-README ב-`python/iroha_python`
מדריך לסטטוס והערות הגירה העדכניות ביותר.