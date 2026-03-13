---
lang: uz
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC xavfsizlik nazorati nazorat ro'yxati - Android SDK

| Maydon | Qiymat |
|-------|-------|
| Versiya | 0,1 (2026-02-12) |
| Qo'llash doirasi | Android SDK + operator asboblari Yaponiya moliyaviy joylashtirishlarida foydalaniladi |
| Egalari | Muvofiqlik va qonunchilik (Daniel Park), Android dasturi rahbari |

## Boshqarish matritsasi

| FISC nazorati | Amalga oshirish tafsilotlari | Dalillar / Ma'lumotnomalar | Holati |
|-------------|-----------------------|-----------------------|--------|
| **Tizim konfiguratsiyasi yaxlitligi** | `ClientConfig` manifest xeshlash, sxemani tekshirish va faqat o'qish uchun ishlash vaqtiga kirishni ta'minlaydi. Konfiguratsiyani qayta yuklashda xatoliklar runbookda hujjatlashtirilgan `android.telemetry.config.reload` hodisalarini chiqaradi. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Amalga oshirildi |
| **Kirish nazorati va autentifikatsiya** | SDK Torii TLS siyosatlari va `/v2/pipeline` imzolangan soʻrovlarini hurmat qiladi; operator ish oqimlari ma'lumotnomasi eskalatsiya uchun qo'llab-quvvatlash Playbook §4–5 va imzolangan Norito artefaktlari orqali o'tishni bekor qilish. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (ish jarayonini bekor qilish). | ✅ Amalga oshirildi |
| **Kriptografik kalitlarni boshqarish** | StrongBox afzal ko'rgan provayderlar, attestatsiyani tekshirish va qurilma matritsasi qamrovi KMS muvofiqligini ta'minlaydi. Attestatsiya jabduqlari natijalari `artifacts/android/attestation/` ostida arxivlangan va tayyorlik matritsasida kuzatilgan. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Amalga oshirildi |
| **Ro‘yxatga olish, kuzatish va saqlash** | Telemetriyani tahrirlash siyosati nozik maʼlumotlarni xeshlaydi, qurilma atributlarini paqirlaydi va saqlashni talab qiladi (7/30/90/365 kunlik oynalar). Qo'llab-quvvatlash o'yin kitobi §8 asboblar paneli chegaralarini tavsiflaydi; `telemetry_override_log.md` da qayd etilgan bekor qilish. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Amalga oshirildi |
| **Operatsiyalar va o'zgarishlarni boshqarish** | GA kesish jarayoni (Qo'llab-quvvatlash Playbook §7.2) va `status.md` yangilanishlari relizga tayyorlikni kuzatish. `docs/source/compliance/android/eu/sbom_attestation.md` orqali bog'langan dalillar (SBOM, Sigstore to'plamlari). | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Amalga oshirildi |
| **Hodisaga javob berish va hisobot berish** | O'yin kitobi jiddiylik matritsasi, SLA javob oynalari va muvofiqlik bildirishnomalarini belgilaydi; telemetriyani bekor qilish + tartibsizlik mashqlari uchuvchilar oldida takrorlanishini ta'minlaydi. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Amalga oshirildi |
| **Ma'lumotlar rezidentligi/lokalizatsiya** | JP-ni joylashtirish uchun telemetriya kollektorlari tasdiqlangan Tokio hududida ishlaydi; StrongBox attestatsiya toʻplamlari mintaqada saqlanadi va hamkor chiptalaridan havola qilinadi. Lokalizatsiya rejasi beta-versiyadan oldin (AND5) yapon tilida hujjatlar mavjudligini taʼminlaydi. | `docs/source/android_support_playbook.md` 9-§; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Davom etmoqda (mahalliylashtirish davom etmoqda) |

## Sharhlovchi eslatmalari

- Regulyatsiya qilingan hamkorni ishga tushirishdan oldin Galaxy S23/S24 uchun qurilma matritsasi yozuvlarini tekshiring (`s23-strongbox-a`, `s24-strongbox-a` tayyorlik hujjati qatorlariga qarang).
- JP o'rnatishdagi telemetriya kollektorlari DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) da belgilangan bir xil saqlash/bekor qilish mantiqini tatbiq etishiga ishonch hosil qiling.
- Bank hamkorlari ushbu nazorat ro'yxatini ko'rib chiqqandan so'ng, tashqi auditorlardan tasdiqlovni oling.