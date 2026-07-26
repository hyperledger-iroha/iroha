---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK tezkor ishga tushirish

Python SDK (`iroha-python`) Rust mijoz yordamchilarini aks ettiradi.
Torii bilan skriptlar, noutbuklar yoki veb-versiyalardan o'zaro aloqada bo'ling. Bu tezkor boshlanish
o'rnatish, tranzaktsiyalarni yuborish va voqealar oqimini o'z ichiga oladi. Chuqurroq uchun
qamrov omborida `python/iroha_python/README.md` ga qarang.

## 1. O'rnatish

```bash
pip install iroha-python
```

Ixtiyoriy qo'shimchalar:

- `pip install aiohttp`, agar siz asinxron variantlarni ishga tushirishni rejalashtirmoqchi bo'lsangiz
  oqim yordamchilari.
- `pip install pynacl`, SDK dan tashqarida Ed25519 kalitini olish kerak bo'lganda.

## 2. Mijoz va imzolovchilarni yarating

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

`ToriiClient` qo'shimcha kalit so'z argumentlarini qabul qiladi, masalan, `timeout_ms`,
`max_retries` va `tls_config`. Yordamchi `resolve_torii_client_config`
Rust CLI bilan tenglikni xohlasangiz, JSON konfiguratsiya foydali yukini tahlil qiladi.

## 3. Tranzaksiyani yuboring

SDK ko'rsatmalar quruvchilar va tranzaksiya yordamchilarini yuboradi, shuning uchun siz kamdan-kam hollarda tuzasiz
Norito foydali yuklarni qo'lda:

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

`build_and_submit_transaction` imzolangan konvertni ham, oxirgisini ham qaytaradi
kuzatilgan holat (masalan, `Committed`, `Rejected`). Agar sizda allaqachon imzolangan bo'lsa
tranzaksiya konvertida `client.submit_transaction_envelope(envelope)` yoki
JSON-markazli `submit_transaction_json`.

## 4. So'rov holati

Barcha REST so'nggi nuqtalarida JSON yordamchilari mavjud va ko'plab kiritilgan ma'lumotlar sinflari mavjud. uchun
Masalan, domenlar ro'yxati:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Sahifani biladigan yordamchilar (masalan, `list_accounts_typed`) ob'ektni qaytaradi
`items` va `next_cursor` ikkalasini ham o'z ichiga oladi.

## 5. Voqealarni translatsiya qilish

Torii SSE so'nggi nuqtalari generatorlar orqali ochiladi. SDK avtomatik ravishda davom etadi
qachon `resume=True` va siz `EventCursor` taqdim etasiz.

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

Boshqa qulaylik usullari orasida `stream_pipeline_transactions`,
`stream_events` (yozilgan filtr quruvchilar bilan) va `stream_verifying_key_events`.

## 6. Keyingi qadamlar

- `python/iroha_python/src/iroha_python/examples/` ostidagi misollarni o'rganing
  boshqaruv, ISO ko'prik yordamchilari va Connectni qamrab oluvchi uchdan-end oqimlar uchun.
- `create_torii_client` / `resolve_torii_client_config` dan foydalaning
  mijozni `iroha_config` JSON fayli yoki muhitidan yuklash.
- Norito RPC yoki Connect-ga xos API uchun maxsus modullarni tekshiring, masalan
  `iroha_python.norito_rpc` va `iroha_python.connect`.

Ushbu qurilish bloklari bilan siz yozmasdan Python'dan Torii mashq qilishingiz mumkin
o'zingizning HTTP elimingiz yoki Norito kodeklaringiz. SDK etuk bo'lganda, qo'shimcha yuqori daraja
quruvchilar qo'shiladi; `python/iroha_python` da README bilan maslahatlashing
oxirgi holat va migratsiya qaydlari uchun katalog.