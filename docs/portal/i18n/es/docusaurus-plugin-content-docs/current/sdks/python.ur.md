---
lang: es
direction: ltr
source: docs/portal/docs/sdks/python.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T12:29:51.735640+00:00"
translation_last_reviewed: 2026-01-30
---

# Python SDK کوئک اسٹارٹ

Python SDK (`iroha-python`) Rust کلائنٹ ہیلپرز کی عکاسی کرتا ہے تاکہ آپ اسکرپٹس، نوٹ بکس، یا ویب بیک اینڈز سے Torii کے ساتھ انٹریکٹ کر سکیں۔ یہ کوئک اسٹارٹ انسٹالیشن، ٹرانزیکشن سبمیشن، اور ایونٹ اسٹریمنگ کا احاطہ کرتا ہے۔ مزید تفصیل کے لیے ریپوزٹری میں `python/iroha_python/README.md` دیکھیں۔

## 1. انسٹال کریں

```bash
pip install iroha-python
```

اختیاری اضافے:

- اگر آپ اسٹریمنگ ہیلپرز کے async ویریئنٹس چلانا چاہتے ہیں تو `pip install aiohttp`.
- جب آپ کو SDK کے باہر Ed25519 key derivation چاہیے تو `pip install pynacl`.

## 2. کلائنٹ اور signers بنائیں

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

`ToriiClient` `timeout_ms`، `max_retries` اور `tls_config` جیسے اضافی آرگومنٹس قبول کرتا ہے۔ اگر آپ Rust CLI کے ساتھ parity چاہتے ہیں تو `resolve_torii_client_config` JSON کنفیگ پے لوڈ پارس کرتا ہے۔

## 3. ٹرانزیکشن سبمٹ کریں

SDK میں instruction builders اور transaction helpers شامل ہیں تاکہ آپ کو Norito payloads ہاتھ سے بنانے کی کم ہی ضرورت پڑے:

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

`build_and_submit_transaction` signed envelope اور آخری مشاہدہ شدہ status (مثلاً `Committed`, `Rejected`) دونوں واپس کرتا ہے۔ اگر آپ کے پاس پہلے سے signed envelope ہے تو `client.submit_transaction_envelope(envelope)` یا JSON‑centric `submit_transaction_json` استعمال کریں۔

## 4. اسٹیٹ کوئری کریں

تمام REST endpoints کے لیے JSON helpers موجود ہیں اور کئی typed dataclasses بھی فراہم کرتے ہیں۔ مثال کے طور پر domains لسٹ کرنا:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Pagination والے helpers (مثلاً `list_accounts_typed`) ایک ایسا آبجیکٹ واپس کرتے ہیں جس میں `items` اور `next_cursor` شامل ہوتے ہیں۔

Account inventory helpers ایک optional `asset_id` فلٹر قبول کرتے ہیں جب آپ صرف کسی مخصوص asset میں دلچسپی رکھتے ہوں:

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
## 6. ایونٹس اسٹریم کریں

Torii SSE endpoints generators کے ذریعے دستیاب ہیں۔ SDK خودکار طور پر resume کرتا ہے جب `resume=True` ہو اور آپ `EventCursor` فراہم کریں۔

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

دیگر سہولت والے طریقوں میں `stream_pipeline_transactions`, `stream_events` (typed filter builders کے ساتھ)، اور `stream_verifying_key_events` شامل ہیں۔

## 7. اگلے اقدامات

- `python/iroha_python/src/iroha_python/examples/` کے تحت مثالیں دیکھیں جن میں governance، ISO bridge helpers، اور Connect کے end‑to‑end flows شامل ہیں۔
- جب آپ `iroha_config` JSON فائل یا environment سے کلائنٹ بوٹ اسٹرپ کرنا چاہیں تو `create_torii_client` / `resolve_torii_client_config` استعمال کریں۔
- Norito RPC یا Connect‑specific APIs کے لیے `iroha_python.norito_rpc` اور `iroha_python.connect` جیسے ماڈیولز دیکھیں۔

## متعلقہ Norito مثالیں

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — اس کوئک اسٹارٹ کے compile/run فلو کی عکاسی کرتا ہے تاکہ آپ Python سے وہی starter contract ڈیپلائے کر سکیں۔
- [Register domain and mint assets](../norito/examples/register-and-mint) — اوپر والے domain + asset flows کے مطابق ہے اور اس وقت مفید ہے جب آپ SDK builders کے بجائے ledger‑side implementation چاہتے ہوں۔
- [Transfer asset between accounts](../norito/examples/transfer-asset) — `transfer_asset` syscall دکھاتا ہے تاکہ آپ contract‑driven transfers کو Python helpers کے ساتھ موازنہ کر سکیں۔

ان building blocks کے ساتھ آپ Python سے Torii استعمال کر سکتے ہیں بغیر اپنے HTTP glue یا Norito codecs لکھے۔ جیسے جیسے SDK بہتر ہوتا ہے، مزید high‑level builders شامل کیے جائیں گے؛ تازہ ترین اسٹیٹس اور migration نوٹس کے لیے `python/iroha_python` ڈائریکٹری کا README دیکھیں۔
