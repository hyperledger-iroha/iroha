---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-12-29T18:16:35.115752+00:00"
translation_last_reviewed: 2026-02-07
title: Norito-RPC Overview
translator: machine-google-reviewed
---

# Norito-RPC umumiy ko'rinishi

Norito-RPC Torii API uchun ikkilik transportdir. U bir xil HTTP yo'llarini qayta ishlatadi
`/v1/pipeline` sifatida, lekin sxemani o'z ichiga olgan Norito ramkali foydali yuklarni almashtiradi
xeshlar va nazorat summalari. Deterministik, tasdiqlangan javoblar kerak bo'lganda foydalaning yoki
quvur liniyasi JSON javoblari muammoga aylanganda.

## Nima uchun almashtiring?
- CRC64 va sxema xeshlari bilan deterministik ramkalash dekodlash xatolarini kamaytiradi.
- SDKlar bo'ylab umumiy Norito yordamchilari mavjud ma'lumotlar modeli turlaridan qayta foydalanishga imkon beradi.
- Torii telemetriyada Norito seanslarini allaqachon teglar, shuning uchun operatorlar kuzatishi mumkin
taqdim etilgan asboblar paneli bilan qabul qilish.

## So'rov yuborish

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. Foydali yukingizni Norito kodek (`iroha_client`, SDK yordamchilari yoki) bilan seriyali qiling.
   `norito::to_bytes`).
2. So'rovni `Content-Type: application/x-norito` bilan yuboring.
3. `Accept: application/x-norito` yordamida Norito javobini so'rang.
4. Mos keladigan SDK yordamchisi yordamida javobni dekodlang.

SDK-ga xos ko'rsatmalar:
- **Rust**: `iroha_client::Client` siz sozlaganingizda avtomatik ravishda Norito bilan kelishib oladi
  `Accept` sarlavhasi.
- **Python**: `iroha_python.norito_rpc` dan `NoritoRpcClient` dan foydalaning.
- **Android**: `NoritoRpcClient` va `NoritoRpcRequestOptions` dan foydalaning.
  Android SDK.
- **JavaScript/Swift**: yordamchilar `docs/source/torii/norito_rpc_tracker.md` da kuzatiladi
  va NRPC-3 tarkibiga kiradi.

## Sinab ko'ring konsol namunasi

Ishlab chiquvchilar portali Try It proksi-serverini yuboradi, shuning uchun sharhlovchilar Noritoni takrorlashlari mumkin
buyurtma skriptlarni yozmasdan foydali yuklar.

1. [Proksi-serverni ishga tushiring](./try-it.md#start-the-proxy-locally) va sozlang
   `TRYIT_PROXY_PUBLIC_URL`, shuning uchun vidjetlar trafikni qaerga yuborishni bilishadi.
2. Ushbu sahifadagi **Sinab ko'ring** kartasini yoki `/reference/torii-swagger`
   For MCP/agent flows, use `/reference/torii-mcp`.
   paneli va `POST /v1/pipeline/submit` kabi oxirgi nuqtani tanlang.
3. **Content-Type** ni `application/x-norito` ga almashtiring, **Binary** ni tanlang.
   muharriri va `fixtures/norito_rpc/transfer_asset.norito` yuklang
   (yoki ro'yxatda ko'rsatilgan har qanday foydali yuk
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. OAuth qurilma kodi vidjeti yoki qo‘lda token orqali tashuvchi tokenini taqdim eting
   maydoni (proksi-server bilan sozlanganda `X-TryIt-Auth` bekor qilishni qabul qiladi
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. So‘rovni yuboring va Torii ro‘yxatda keltirilgan `schema_hash` bilan mos kelishini tekshiring.
   `fixtures/norito_rpc/schema_hashes.json`. Mos keladigan xeshlar buni tasdiqlaydi
   Norito sarlavhasi brauzer/proksi-serverdan omon qoldi.

“Yoʻl xaritasi” boʻyicha dalil olish uchun “Try It” skrinshotini ishga tushirish bilan bogʻlang
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Skript o'raladi
`cargo xtask norito-rpc-verify`, JSON xulosasini yozadi
`artifacts/norito_rpc/<timestamp>/` va bir xil moslamalarni ushlaydi
portal iste'mol qilinadi.

## Nosozliklarni bartaraf etish

| Alomat | Qayerda paydo bo'ladi | Ehtimoliy sabab | Tuzatish |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii javob | `Content-Type` sarlavhasi etishmayotgan yoki noto'g'ri | Foydali yukni yuborishdan oldin `Content-Type: application/x-norito` ni o'rnating. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii javob tanasi/sarlavhalari | Fikstur sxemasi xeshi Torii tuzilishidan farq qiladi | `cargo xtask norito-rpc-fixtures` bilan armaturalarni qayta tiklang va `fixtures/norito_rpc/schema_hashes.json` da xeshni tasdiqlang; agar oxirgi nuqta hali Norito ni yoqmagan bo'lsa, JSONga qayting. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Sinab ko'ring proksi javob | So'rov `TRYIT_PROXY_ALLOWED_ORIGINS` | ro'yxatida bo'lmagan manbadan kelgan Portal manbasini (masalan, `https://docs.devnet.sora.example`) env var ga qo'shing va proksi-serverni qayta ishga tushiring. |
| `{"error":"rate_limited"}` (HTTP 429) | Sinab ko'ring proksi javob | IP uchun kvota `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` byudjetidan oshib ketdi | Ichki yuk sinovi uchun chegarani oshiring yoki oyna qayta tiklanmaguncha kuting (JSON javobida `retryAfterMs` ga qarang). |
| `{"error":"upstream_timeout"}` (HTTP 504) yoki `{"error":"upstream_error"}` (HTTP 502) | Sinab ko'ring proksi javob | Torii vaqti tugadi yoki proksi-server sozlangan backendga kira olmadi | `TRYIT_PROXY_TARGET` ulanish imkoniyati mavjudligini tekshiring, Torii holatini tekshiring yoki kattaroq `TRYIT_PROXY_TIMEOUT_MS` bilan qayta urinib ko'ring. |

Koʻproq sinab koʻring diagnostika va OAuth maslahatlari mavjud
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Qo'shimcha manbalar
- Transport RFC: `docs/source/torii/norito_rpc.md`
- Ijroiya xulosasi: `docs/source/torii/norito_rpc_brief.md`
- Harakat kuzatuvchisi: `docs/source/torii/norito_rpc_tracker.md`
- Sinab ko'ring proksi-server ko'rsatmalari: `docs/portal/docs/devportal/try-it.md`
