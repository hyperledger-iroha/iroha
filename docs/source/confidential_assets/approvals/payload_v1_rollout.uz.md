---
lang: uz
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Foydali yuk v1 ishlab chiqarishni tasdiqlash (SDK kengashi, 2026-04-28).
//!
//! `roadmap.md:M1` tomonidan talab qilingan SDK Kengashi qarori eslatmasini oladi, shuning uchun
//! shifrlangan foydali yuk v1 chiqarilishi tekshiriladigan yozuvga ega (etkazib beriladigan M1.4).

# Payload v1 ishga tushirish qarori (2026-04-28)

- **Rais:** SDK Kengashi rahbari (M. Takemiya)
- **Ovoz berish a'zolari:** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- **Kuzatuvchilar:** Dastur Mgmt, Telemetriya Ops

## Kirishlar ko'rib chiqildi

1. **Swift ulanishlar va jo‘natuvchilar** — `ShieldRequest`/`UnshieldRequest`, asinxron yuboruvchilar va Tx quruvchi yordamchilari paritet testlari va hujjatlar.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI ergonomikasi** — `iroha app zk envelope` yordamchisi ish oqimlarini kodlash/tekshirish, shuningdek, yo‘l xaritasi ergonomikasi talabiga muvofiq nosozlik diagnostikasini o‘z ichiga oladi.【crates/iroha_cli/src/zk.rs:1256】
3. **Deterministik moslamalar va paritet toʻplamlari** — umumiy moslama + Norito bayt/xato yuzalarini saqlash uchun Rust/Swift tekshiruvi hizalangan.【Fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Qaror

- **SDK va CLI uchun foydali yuk v1 versiyasini tasdiqlang**, bu Swift hamyonlariga maxsus sanitariya-tesisatsiz maxfiy konvertlarni yaratish imkonini beradi.
- **Shartlar:** 
  - Paritet moslamalarini CI drift ogohlantirishlari ostida saqlang (`scripts/check_norito_bindings_sync.py` bilan bog'langan).
  - Operatsion o'yin kitobini `docs/source/confidential_assets.md` da hujjatlashtiring (allaqachon Swift SDK PR orqali yangilangan).
  - Har qanday ishlab chiqarish bayroqlarini o'zgartirishdan oldin kalibrlash + telemetriya dalillarini yozib oling (M2 ostida kuzatilgan).

## Harakat elementlari

| Egasi | Element | Muddati |
|-------|------|-----|
| Swift Lead | GA mavjudligini e'lon qiling + README parchalari | 2026-05-01 |
| CLI Maintainer | `iroha app zk envelope --from-fixture` yordamchisini qo'shing (ixtiyoriy) | Orqaga chiqish (bloklanmaydi) |
| DevRel WG | Payload v1 ko'rsatmalari bilan hamyonni tezkor ishga tushirishni yangilang | 2026-05-05 |

> **Izoh:** Ushbu eslatma `roadmap.md:2426` da vaqtinchalik “kengash ma’qullashini kutayotgan” chaqiruvining o‘rnini bosadi va M1.4 kuzatuvchi bandini qondiradi. Harakatning keyingi elementlari yopilganda `status.md` ni yangilang.