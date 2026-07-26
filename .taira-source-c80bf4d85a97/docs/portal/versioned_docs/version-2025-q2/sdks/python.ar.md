---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# بايثون SDK البدء السريع

يعكس Python SDK (`iroha-python`) مساعدي عميل Rust حتى تتمكن من ذلك
التفاعل مع Torii من البرامج النصية أو دفاتر الملاحظات أو واجهات الويب الخلفية. هذه البداية السريعة
يغطي التثبيت، وتقديم المعاملات، وتدفق الأحداث. لأعمق
التغطية راجع `python/iroha_python/README.md` في المستودع.

## 1. التثبيت

```bash
pip install iroha-python
```

الإضافات الاختيارية:

- `pip install aiohttp` إذا كنت تخطط لتشغيل المتغيرات غير المتزامنة لـ
  مساعدين التدفق.
- `pip install pynacl` عندما تحتاج إلى اشتقاق مفتاح Ed25519 خارج SDK.

## 2. أنشئ عميلاً وموقعين

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

يقبل `ToriiClient` وسيطات كلمات رئيسية إضافية مثل `timeout_ms`،
`max_retries` و`tls_config`. المساعد `resolve_torii_client_config`
يوزع حمولة تكوين JSON إذا كنت تريد التكافؤ مع Rust CLI.

## 3. أرسل المعاملة

تقوم حزمة SDK بشحن منشئي التعليمات ومساعدي المعاملات، لذا نادرًا ما تقوم بالإنشاء
حمولات Norito يدويًا:

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

يقوم `build_and_submit_transaction` بإرجاع كل من المظروف الموقع والأخير
الحالة الملحوظة (على سبيل المثال، `Committed`، `Rejected`). إذا كان لديك توقيع بالفعل
استخدم مظروف المعاملة `client.submit_transaction_envelope(envelope)` أو
JSON تتمحور حول `submit_transaction_json`.

## 4. حالة الاستعلام

تحتوي جميع نقاط نهاية REST على مساعدات JSON والعديد منها يعرض فئات البيانات المكتوبة. ل
على سبيل المثال، إدراج المجالات:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

يقوم المساعدون الذين يدركون ترقيم الصفحات (على سبيل المثال، `list_accounts_typed`) بإرجاع كائن
يحتوي على كلاً من `items` و`next_cursor`.

## 5. بث الأحداث

يتم كشف نقاط النهاية Torii SSE عبر المولدات. يتم استئناف SDK تلقائيًا
عندما `resume=True` وتقوم بتوفير `EventCursor`.

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

تتضمن طرق الراحة الأخرى `stream_pipeline_transactions`،
`stream_events` (مع منشئي المرشحات المكتوبة)، و`stream_verifying_key_events`.

## 6. الخطوات التالية

- استكشف الأمثلة ضمن `python/iroha_python/src/iroha_python/examples/`
  للتدفقات الشاملة التي تغطي الإدارة ومساعدي جسر ISO والاتصال.
- استخدم `create_torii_client` / `resolve_torii_client_config` عندما تريد ذلك
  قم بتمهيد العميل من ملف أو بيئة `iroha_config` JSON.
- بالنسبة إلى Norito RPC أو واجهات برمجة التطبيقات الخاصة بالاتصال، تحقق من الوحدات النمطية المتخصصة مثل
  `iroha_python.norito_rpc` و`iroha_python.connect`.

باستخدام هذه العناصر الأساسية، يمكنك ممارسة Torii من Python دون الكتابة
الغراء HTTP الخاص بك أو برامج الترميز Norito. ومع نضوج SDK، سيتم إضافة المزيد من المستوى العالي
سيتم إضافة بناة. راجع الملف التمهيدي الموجود في `python/iroha_python`
الدليل للحصول على أحدث ملاحظات الحالة والهجرة.