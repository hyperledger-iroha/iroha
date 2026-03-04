---
lang: uz
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
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

Hisob inventarizatsiyasi yordamchilari ixtiyoriy `asset_id` filtrini faqat siz qabul qilganingizda qabul qiladi
Muayyan aktivga g'amxo'rlik qilish:

```python
asset_id = "rose#wonderland#alice@test"
assets = client.list_account_assets("alice@test", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("alice@test", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("rose#wonderland", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Oflayn nafaqalar

Hamyon sertifikatlarini chiqarish va ro'yxatdan o'tish uchun oflayn ruxsat so'nggi nuqtalaridan foydalaning
ular daftarda. `top_up_offline_allowance` muammoni + ro'yxatga olish bosqichlarini zanjirlaydi
(yagona to'ldirishning yakuniy nuqtasi yo'q):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "ih58:...",
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
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

Yangilash uchun joriy sertifikat identifikatori bilan `top_up_offline_allowance_renewal` raqamiga qo‘ng‘iroq qiling:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Agar oqimni ajratish kerak bo'lsa, `issue_offline_certificate` (yoki
`issue_offline_certificate_renewal`) keyin `register_offline_allowance`
yoki `renew_offline_allowance`.

## 6. Voqealarni translatsiya qilish

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

## 7. Keyingi qadamlar

- `python/iroha_python/src/iroha_python/examples/` ostidagi misollarni o'rganing
  boshqaruv, ISO ko'prik yordamchilari va Connectni qamrab oluvchi uchdan-end oqimlar uchun.
- `create_torii_client` / `resolve_torii_client_config` dan foydalaning
  mijozni `iroha_config` JSON fayli yoki muhitidan yuklash.
- Norito RPC yoki Connect-ga xos API uchun maxsus modullarni tekshiring, masalan
  `iroha_python.norito_rpc` va `iroha_python.connect`.

## Tegishli Norito misollari

- [Hajimari kirish nuqtasi skeleti](../norito/examples/hajimari-entrypoint) - kompilyatsiya/ishlash jarayonini aks ettiradi
  Python'dan bir xil boshlang'ich shartnomasini o'rnatishingiz uchun ushbu tezkor ishga tushirishdan ish oqimi.
- [Domen va zarb aktivlarini ro'yxatdan o'tkazish](../norito/examples/register-and-mint) - domenga mos keladi +
  aktivlar yuqorida harakat qiladi va SDK quruvchilari oʻrniga daftar tomonida amalga oshirishni xohlasangiz foydali boʻladi.
- [Aktivni hisoblar o'rtasida o'tkazish](../norito/examples/transfer-asset) - `transfer_asset` ni namoyish etadi
  syscall-ni ishlating, shuning uchun siz shartnoma asosidagi o'tkazmalarni Python yordamchi usullari bilan taqqoslashingiz mumkin.

Ushbu qurilish bloklari bilan siz yozmasdan Python'dan Torii mashq qilishingiz mumkin
o'zingizning HTTP elimingiz yoki Norito kodeklaringiz. SDK etuk bo'lganda, qo'shimcha yuqori daraja
quruvchilar qo'shiladi; `python/iroha_python` da README bilan maslahatlashing
oxirgi holat va migratsiya qaydlari uchun katalog.