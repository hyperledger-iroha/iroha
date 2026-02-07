---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ازگر ایس ڈی کے کوئیک اسٹارٹ

ازگر ایس ڈی کے (`iroha-python`) زنگ کلائنٹ کے مددگاروں کو آئینہ دار کرتا ہے تاکہ آپ کر سکیں
اسکرپٹ ، نوٹ بک ، یا ویب بیک اینڈ سے Torii کے ساتھ بات چیت کریں۔ یہ کوئک اسٹارٹ
انسٹالیشن ، ٹرانزیکشن جمع کرانے ، اور ایونٹ اسٹریمنگ کا احاطہ کرتا ہے۔ گہری کے لئے
کوریج ریپوزٹری میں `python/iroha_python/README.md` دیکھیں۔

## 1۔ انسٹال کریں

```bash
pip install iroha-python
```

اختیاری ایکسٹرا:

- `pip install aiohttp` اگر آپ اسنکرونوس مختلف حالتوں کو چلانے کا ارادہ رکھتے ہیں
  اسٹریمنگ مددگار۔
- `pip install pynacl` جب آپ کو SDK سے باہر ED25519 کلیدی مشتق کی ضرورت ہو۔

## 2۔ ایک مؤکل اور دستخط کرنے والے بنائیں

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

`ToriiClient` اضافی مطلوبہ الفاظ کے دلائل کو قبول کرتا ہے جیسے `timeout_ms` ،
`max_retries` ، اور `tls_config`۔ مددگار `resolve_torii_client_config`
اگر آپ مورچا CLI کے ساتھ برابری چاہتے ہیں تو JSON کنفیگریشن پے لوڈ کو پارس کرتا ہے۔

## 3. ٹرانزیکشن جمع کروائیں

ایس ڈی کے جہازوں کو ہدایت کاروں اور ٹرانزیکشن مددگاروں کی ہدایت کرتا ہے تاکہ آپ شاذ و نادر ہی تعمیر کریں
Norito پے لوڈ ہاتھ سے:

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

`build_and_submit_transaction` دستخط شدہ لفافے اور آخری دونوں کو لوٹاتا ہے
مشاہدہ شدہ حیثیت (جیسے ، `Committed` ، `Rejected`)۔ اگر آپ کے پاس پہلے سے ہی دستخط ہوچکے ہیں
ٹرانزیکشن لفافہ `client.submit_transaction_envelope(envelope)` یا The استعمال کریں
JSON-سینٹرک `submit_transaction_json`۔

## 4۔ استفسار کی حالت

تمام آرام کے اختتامی مقامات میں JSON مددگار اور بہت سے ٹائپ شدہ ڈیٹا کلاک کو بے نقاب کرتے ہیں۔ کے لئے
مثال کے طور پر ، ڈومینز کی فہرست:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

صفحہ بندی سے واقف مددگار (جیسے ، `list_accounts_typed`) کسی شے کو واپس کریں
`items` اور `next_cursor` دونوں پر مشتمل ہے۔

## 5. اسٹریم واقعات

Torii SSE اختتامی نکات جنریٹرز کے ذریعہ بے نقاب ہیں۔ SDK خود بخود دوبارہ شروع ہوجاتا ہے
جب `resume=True` اور آپ `EventCursor` فراہم کرتے ہیں۔

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

سہولت کے دیگر طریقوں میں `stream_pipeline_transactions` شامل ہیں ،
`stream_events` (ٹائپ شدہ فلٹر بلڈروں کے ساتھ) ، اور `stream_verifying_key_events`۔

## 6. اگلے اقدامات

- `python/iroha_python/src/iroha_python/examples/` کے تحت مثالوں کو دریافت کریں
  گورننس ، آئی ایس او برج مددگار ، اور کنیکٹ کو ڈھکنے والے اختتام سے آخر میں بہاؤ کے ل .۔
- جب آپ چاہیں تو `create_torii_client` / `resolve_torii_client_config` استعمال کریں
  `iroha_config` JSON فائل یا ماحول سے کلائنٹ کو بوٹسٹریپ کریں۔
- Norito RPC یا کنیکٹ سے متعلق مخصوص APIs کے لئے ، خصوصی ماڈیولز کی جانچ کریں جیسے
  `iroha_python.norito_rpc` اور `iroha_python.connect`۔

ان بلڈنگ بلاکس کے ذریعہ آپ بغیر لکھے بغیر ازگر سے Torii ورزش کرسکتے ہیں
آپ کا اپنا HTTP گلو یا Norito کوڈیکس۔ جیسا کہ ایس ڈی کے پختہ ہوتا ہے ، اضافی اعلی سطح
بلڈروں کو شامل کیا جائے گا۔ `python/iroha_python` میں README سے مشورہ کریں
تازہ ترین حیثیت اور ہجرت کے نوٹ کے لئے ڈائرکٹری۔