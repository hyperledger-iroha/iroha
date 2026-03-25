---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 33ea43221e6fd7921d963d7f5898b2f2518a2232ee971d13d231f9b65631e2b4
source_last_modified: "2026-01-30T15:41:27+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# البدء السريع لـ SDK Python

يعكس SDK الخاص بـ Python (`iroha-python`) مساعدات عميل Rust حتى تتمكن من التفاعل مع Torii من السكربتات أو الدفاتر أو الواجهات الخلفية. يغطي هذا البدء السريع التثبيت، إرسال المعاملات، وبث الأحداث. لمزيد من التفاصيل راجع `python/iroha_python/README.md` في المستودع.

## 1. التثبيت

```bash
pip install iroha-python
```

إضافات اختيارية:

- `pip install aiohttp` إذا كنت تخطط لتشغيل النسخ غير المتزامنة من مساعدات البث.
- `pip install pynacl` عندما تحتاج إلى اشتقاق مفاتيح Ed25519 خارج الـ SDK.

## 2. إنشاء عميل وموقّعين

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

`ToriiClient` يقبل معاملات إضافية مثل `timeout_ms` و`max_retries` و`tls_config`. يقوم `resolve_torii_client_config` بتحليل حمولة إعدادات JSON إذا رغبت في التكافؤ مع CLI الخاصة بـ Rust.

## 3. إرسال معاملة

يوفر الـ SDK بُنّاء تعليمات ومساعدات معاملات بحيث نادرًا ما تحتاج لبناء حمولة Norito يدويًا:

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

`build_and_submit_transaction` يعيد كلًا من الـ envelope الموقّع وآخر حالة مرصودة (مثل `Committed` أو `Rejected`). إذا كان لديك envelope موقّع بالفعل، استخدم `client.submit_transaction_envelope(envelope)` أو `submit_transaction_json` المعتمد على JSON.

## 4. استعلام الحالة

كل نقاط REST لها مساعدات JSON وكثير منها يعرض dataclasses مُنَمَّطة. على سبيل المثال، سرد النطاقات:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

المساعدات ذات الترقيم (مثل `list_accounts_typed`) تعيد كائنًا يحتوي على `items` و`next_cursor`.

مساعدات جرد الحساب تقبل فلترًا اختياريًا `asset_id` عندما تهتم بأصل محدد فقط:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline allowances

استخدم نقاط offline allowance لإصدار شهادات المحافظ وتسجيلها على السلسلة. `top_up_offline_allowance` يربط خطوات الإصدار + التسجيل (لا يوجد endpoint موحّد للتعبئة):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "i105:...",
    "allowance": {"asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

للتجديدات، استدعِ `top_up_offline_allowance_renewal` مع معرف الشهادة الحالية:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

إذا احتجت تقسيم التدفق، استدعِ `issue_offline_certificate` (أو `issue_offline_certificate_renewal`) ثم `register_offline_allowance` أو `renew_offline_allowance`.

## 6. بث الأحداث

تُعرض نقاط SSE في Torii عبر مولدات. يستأنف الـ SDK تلقائيًا عندما يكون `resume=True` وتقدّم `EventCursor`.

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

تشمل طرق الراحة الأخرى `stream_pipeline_transactions` و`stream_events` (مع بُنّاء فلاتر مُنمَّطة) و`stream_verifying_key_events`.

## 7. الخطوات التالية

- استكشف الأمثلة تحت `python/iroha_python/src/iroha_python/examples/` لتدفقات شاملة تغطي الحوكمة ومساعدات ISO bridge وConnect.
- استخدم `create_torii_client` / `resolve_torii_client_config` عندما تريد تهيئة العميل من ملف JSON لـ `iroha_config` أو من البيئة.
- لواجهات Norito RPC أو Connect، راجع الوحدات المتخصصة مثل `iroha_python.norito_rpc` و`iroha_python.connect`.

## أمثلة Norito ذات صلة

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — يعكس سير compile/run في هذا البدء السريع حتى تتمكن من نشر نفس العقد الابتدائي من Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — يتطابق مع تدفقات النطاق + الأصل أعلاه وهو مفيد عندما تريد تنفيذ الدفتر بدلًا من بُنّاء الـ SDK.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — يعرض syscall `transfer_asset` لتتمكن من مقارنة التحويلات المدفوعة بالعقود مع مساعدات Python.

بهذه اللبنات يمكنك تشغيل Torii من Python دون كتابة طبقة HTTP أو codecs Norito بنفسك. ومع نضوج الـ SDK ستُضاف بُنّاءات عالية المستوى؛ راجع README في دليل `python/iroha_python` لأحدث الحالة وملاحظات الترحيل.
