---
lang: ru
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e5997ecb958473782164c942a85dcfb6812729c878713a5cf6b704242b08c6
source_last_modified: "2026-01-30T15:41:27+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# מדריך התחלה מהירה ל‑SDK Python

SDK הפייתון (`iroha-python`) משקף את עזרי הלקוח של Rust כך שתוכלו לעבוד עם Torii מסקריפטים, מחברות או backend‑ים. מדריך זה מכסה התקנה, שליחת טרנזקציות וזרימת אירועים. להעמקה ראו `python/iroha_python/README.md` במאגר.

## 1. התקנה

```bash
pip install iroha-python
```

תוספות אופציונליות:

- `pip install aiohttp` אם אתם מתכננים להריץ גרסאות אסינכרוניות של עזרי ה‑streaming.
- `pip install pynacl` כאשר צריך נגזרת מפתחות Ed25519 מחוץ ל‑SDK.

## 2. יצירת לקוח וחתימות

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

`ToriiClient` מקבל ארגומנטים נוספים כמו `timeout_ms`, `max_retries` ו‑`tls_config`. העזר `resolve_torii_client_config` מנתח payload JSON של תצורה אם רוצים פריטי עם ה‑CLI של Rust.

## 3. שליחת טרנזקציה

ה‑SDK מספק builders של הוראות ועזרי טרנזקציות כדי שלא תצטרכו לבנות payloads של Norito ידנית:

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

`build_and_submit_transaction` מחזיר גם envelope חתום וגם את הסטטוס האחרון שנצפה (למשל `Committed`, `Rejected`). אם כבר יש envelope חתום, השתמשו ב‑`client.submit_transaction_envelope(envelope)` או ב‑`submit_transaction_json` הממוקד JSON.

## 4. שאילת מצב

כל נקודות ה‑REST מגיעות עם עזרי JSON, ורבות מהן חושפות dataclasses טיפוסיים. למשל, רשימת דומיינים:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

עזרי פאגינציה (למשל `list_accounts_typed`) מחזירים אובייקט הכולל `items` ו‑`next_cursor`.

עזרי מלאי חשבון מקבלים פילטר `asset_id` אופציונלי כאשר אתם מתעניינים בנכס מסוים בלבד:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline V2 readiness

Use `GET /v1/offline/v2/readiness` through `get_offline_v2_readiness()` for offline feature discovery.
Offline V2 note issuance, redemption, and audit payloads are submitted as transaction instructions;
legacy offline allowance, reserve, revocation, transfer-history, and cash HTTP routes are no longer published by Torii.

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")
readiness = client.get_offline_v2_readiness()
print("offline notes", readiness.offline_note_v2)
```
## 6. זרימת אירועים

נקודות SSE של Torii נחשפות באמצעות גנרטורים. ה‑SDK ממשיך אוטומטית כאשר `resume=True` ואתם מספקים `EventCursor`.

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

שיטות נוחות נוספות כוללות `stream_pipeline_transactions`, `stream_events` (עם builders של פילטרים טיפוסיים) ו‑`stream_verifying_key_events`.

## 7. צעדים הבאים

- בדקו את הדוגמאות תחת `python/iroha_python/src/iroha_python/examples/` עבור זרימות end‑to‑end של governance, ISO bridge ו‑Connect.
- השתמשו ב‑`create_torii_client` / `resolve_torii_client_config` כדי לאתחל לקוח מקובץ JSON של `iroha_config` או מהסביבה.
- עבור APIs של Norito RPC או Connect, בדקו מודולים ייעודיים כמו `iroha_python.norito_rpc` ו‑`iroha_python.connect`.

## דוגמאות Norito קשורות

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — משקף את זרימת compile/run במדריך זה כדי לפרוס את אותו חוזה התחלה מפייתון.
- [Register domain and mint assets](../norito/examples/register-and-mint) — תואם לזרימות דומיין + נכסים לעיל ומועיל כאשר רוצים את מימוש ה‑ledger ולא את builders של ה‑SDK.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — מציג את syscall `transfer_asset` כדי להשוות העברות מבוססות חוזים עם עזרי פייתון.

עם אבני הבניין הללו תוכלו להפעיל את Torii מפייתון בלי לכתוב glue HTTP או Norito codecs משלכם. ככל שה‑SDK מתבגר יתווספו builders ברמה גבוהה יותר; עיינו ב‑README שב‑`python/iroha_python` למצב העדכני ולהערות המיגרציה.
