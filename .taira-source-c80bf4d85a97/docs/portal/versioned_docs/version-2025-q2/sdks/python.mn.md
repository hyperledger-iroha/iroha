---
lang: mn
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK хурдан эхлүүлэх

Python SDK (`iroha-python`) нь Rust клиентийн туслахуудыг тусгадаг тул та
скрипт, дэвтэр эсвэл вэб арын хэсгээс Torii-тэй харилцах. Энэ хурдан эхлэл
суулгах, гүйлгээ илгээх, үйл явдлын урсгалыг хамарна. Илүү гүнзгийрүүлэхийн тулд
хамрах хүрээ нь агуулахаас `python/iroha_python/README.md`-г үзнэ үү.

## 1. Суулгах

```bash
pip install iroha-python
```

Нэмэлт нэмэлтүүд:

- Хэрэв та асинхрон хувилбаруудыг ажиллуулахаар төлөвлөж байгаа бол `pip install aiohttp`
  урсгалын туслахууд.
- SDK-ээс гадуур Ed25519 түлхүүр гарган авах шаардлагатай үед `pip install pynacl`.

## 2. Үйлчлүүлэгч болон гарын үсэг зурсан хүмүүсийг үүсгэ

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

`ToriiClient` нь `timeout_ms` гэх мэт нэмэлт түлхүүр үгийн аргументуудыг хүлээн авдаг.
`max_retries`, `tls_config`. Туслах `resolve_torii_client_config`
Хэрэв та Rust CLI-тай тэнцэхийг хүсвэл JSON тохиргооны ачааллыг задлан шинжилнэ.

## 3. Гүйлгээ илгээх

SDK нь зааварчилгаа бүтээгчид болон гүйлгээний туслахуудыг илгээдэг тул та бүтээх нь ховор
Norito гар аргаар ачаалах:

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

`build_and_submit_transaction` гарын үсэг зурсан дугтуй болон сүүлчийн дугтуйг хоёуланг нь буцаана
ажиглагдсан байдал (жишээ нь, `Committed`, `Rejected`). Хэрэв та аль хэдийн гарын үсэг зурсан бол
гүйлгээний дугтуйнд `client.submit_transaction_envelope(envelope)` эсвэл
JSON төвтэй `submit_transaction_json`.

## 4. Асуулгын төлөв

Бүх REST төгсгөлийн цэгүүд нь JSON туслахуудтай бөгөөд олон төрлийн бичсэн өгөгдлийн ангиудтай. Учир нь
Жишээ нь, домайнуудыг жагсаах:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Хуудасчлахыг мэддэг туслахууд (жишээ нь, `list_accounts_typed`) объектыг буцаадаг.
`items` болон `next_cursor` хоёуланг нь агуулна.

## 5. Үйл явдлыг дамжуулах

Torii SSE төгсгөлийн цэгүүд генераторуудаар дамжин илэрдэг. SDK автоматаар үргэлжилнэ
`resume=True` ба та `EventCursor` өгөх үед.

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

Бусад тохиромжтой аргууд нь `stream_pipeline_transactions`,
`stream_events` (хэвчилсэн шүүлтүүр бүтээгчтэй), `stream_verifying_key_events`.

## 6. Дараагийн алхамууд

- `python/iroha_python/src/iroha_python/examples/` доорх жишээнүүдийг судлаарай
  засаглал, ISO гүүрний туслахууд болон Connect-ийг хамарсан төгсгөл хоорондын урсгалд зориулагдсан.
- Та хүссэн үедээ `create_torii_client` / `resolve_torii_client_config` ашиглана уу.
  клиентийг `iroha_config` JSON файл эсвэл орчноос ачаалах.
- Norito RPC эсвэл Connect-д зориулсан API-ийн хувьд тусгайлсан модулиудыг шалгана уу.
  `iroha_python.norito_rpc` ба `iroha_python.connect`.

Эдгээр барилгын блокуудын тусламжтайгаар та Python-оос Torii дасгалыг бичихгүйгээр хийж болно
өөрийн HTTP цавуу эсвэл Norito кодлогч. SDK боловсорч гүйцсэнээр нэмэлт өндөр түвшний
барилгачид нэмэгдэх болно; `python/iroha_python` дахь README-ээс лавлана уу
хамгийн сүүлийн үеийн статус болон шилжилтийн тэмдэглэлийн лавлах.