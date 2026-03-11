---
lang: mn
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Python SDK хурдан эхлүүлэх

Python SDK (`iroha-python`) нь Rust клиентийн туслахуудыг тусгадаг тул та
Torii-тэй скрипт, тэмдэглэлийн дэвтэр эсвэл вэб арын хэсгээс харьцах. Энэ хурдан эхлэл
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
гүйлгээний дугтуйг ашиглан `client.submit_transaction_envelope(envelope)` эсвэл
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

Дансны бараа материалын туслахууд нь зөвхөн танд байгаа үед нэмэлт `asset_id` шүүлтүүрийг хүлээн авдаг
тодорхой хөрөнгийн талаар санаа тавих:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("rose#wonderland", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Оффлайн тэтгэмж

Түрийвчний гэрчилгээ олгох, бүртгүүлэхийн тулд офлайн тэтгэмжийн төгсгөлийн цэгүүдийг ашиглана уу
тэднийг дэвтэр дээр. `top_up_offline_allowance` асуудал + бүртгүүлэх алхмуудыг гинжлэнэ
(нэг цэнэглэх эцсийн цэг байхгүй):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "i105:...",
    "allowance": {"asset": "usd#wonderland", "amount": "10", "commitment": [1, 2]},
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

Шинэчлэлт хийхийн тулд одоогийн гэрчилгээний дугаартай `top_up_offline_allowance_renewal` руу залгана уу:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Хэрэв та урсгалыг хуваах шаардлагатай бол `issue_offline_certificate` (эсвэл
`issue_offline_certificate_renewal`) дараа нь `register_offline_allowance`
эсвэл `renew_offline_allowance`.

## 6. Үйл явдлуудыг дамжуулах

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

## 7. Дараагийн алхамууд

- `python/iroha_python/src/iroha_python/examples/` доорх жишээнүүдийг судлаарай
  засаглал, ISO гүүрний туслахууд болон Connect-ийг хамарсан төгсгөл хоорондын урсгалд зориулагдсан.
- Та хүссэн үедээ `create_torii_client` / `resolve_torii_client_config` ашиглана уу.
  клиентийг `iroha_config` JSON файл эсвэл орчноос ачаалах.
- Norito RPC эсвэл Connect-д зориулсан API-ийн хувьд тусгайлсан модулиудыг шалгана уу.
  `iroha_python.norito_rpc` болон `iroha_python.connect`.

## Холбогдох Norito жишээнүүд

- [Хажимари нэвтрэх цэгийн араг яс](../norito/examples/hajimari-entrypoint) — эмхэтгэх/ажиллуулах үйлдлийг тусгадаг
  Энэ хурдан эхлэлийн ажлын урсгалыг ашигласнаар та Python-оос ижил эхлэлийн гэрээг ашиглах боломжтой болно.
- [Домэйн болон гаалийн хөрөнгийг бүртгүүлэх](../norito/examples/register-and-mint) — домайн + таарч байна
  хөрөнгийн урсгал нь дээрх бөгөөд SDK бүтээгчдийн оронд дэвтэр талын хэрэгжилтийг хүсэж байгаа үед хэрэг болно.
- [Дансны хооронд хөрөнгө шилжүүлэх](../norito/examples/transfer-asset) — `transfer_asset`-г харуулж байна
  syscall ашиглан гэрээнд суурилсан шилжүүлгийг Python туслах аргуудтай харьцуулах боломжтой.

Эдгээр барилгын блокуудын тусламжтайгаар та Python-оос Torii дасгалыг бичихгүйгээр хийх боломжтой
өөрийн HTTP цавуу эсвэл Norito кодлогч. SDK боловсорч гүйцсэнээр нэмэлт өндөр түвшний
барилгачид нэмэгдэх болно; `python/iroha_python` дахь README-тэй зөвлөлдөнө үү
хамгийн сүүлийн үеийн статус болон шилжилтийн тэмдэглэлийн лавлах.