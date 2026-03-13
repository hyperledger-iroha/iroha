---
lang: uz
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Client API konfiguratsiya ma'lumotnomasi

Ushbu hujjat Torii mijozga qaragan konfiguratsiya tugmalarini kuzatib boradi.
`iroha_config::parameters::user::Torii` orqali yuzalar. Quyidagi bo'lim
NRPC-1 uchun joriy qilingan Norito-RPC transport boshqaruvlariga e'tibor qaratadi; kelajak
mijoz API sozlamalari ushbu faylni kengaytirishi kerak.

### `torii.transport.norito_rpc`

| Kalit | Tur | Standart | Tavsif |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Ikkilik Norito dekodlash imkonini beruvchi asosiy kalit. `false`, Torii, `403 norito_rpc_disabled` bilan har bir Norito-RPC so'rovini rad etadi. |
| `stage` | `string` | `"disabled"` | Chiqarish darajasi: `disabled`, `canary` yoki `ga`. Bosqichlar qabul qilish qarorlarini va `/rpc/capabilities` chiqishini boshqaradi. |
| `require_mtls` | `bool` | `false` | Norito-RPC tashish uchun mTLSni qo'llaydi: `true`, Torii mTLS marker sarlavhasi (masalan, I10000X) bo'lmagan Norito-RPC so'rovlarini rad etadi. Bayroq `/rpc/capabilities` orqali chiqariladi, shuning uchun SDK noto'g'ri sozlangan muhitlar haqida ogohlantirishi mumkin. |
| `allowed_clients` | `array<string>` | `[]` | Kanareykalar ro'yxati. `stage = "canary"` bo'lsa, ushbu ro'yxatda faqat `X-API-Token` sarlavhasi mavjud bo'lgan so'rovlar qabul qilinadi. |

Konfiguratsiyaga misol:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Bosqich semantikasi:

- **o'chirilgan** — Norito-RPC hatto `enabled = true` bo'lsa ham mavjud emas. Mijozlar
  `403 norito_rpc_disabled` qabul qiling.
- **kanarey** — So‘rovlar biriga mos keladigan `X-API-Token` sarlavhasini o‘z ichiga olishi kerak
  `allowed_clients`. Boshqa barcha so'rovlar `403-ni oladi
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC har bir autentifikatsiya qilingan qo'ng'iroq qiluvchi uchun mavjud (shartga muvofiq)
  odatiy tarif va avtoulovdan oldingi chegaralar).

Operatorlar ushbu qiymatlarni `/v2/config` orqali dinamik ravishda yangilashlari mumkin. Har bir o'zgarish
darhol `/rpc/capabilities` da aks ettirilib, SDK va kuzatuvchanlikka imkon beradi
jonli transport holatini ko'rsatish uchun asboblar paneli.