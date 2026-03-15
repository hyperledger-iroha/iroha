---
lang: uz
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2025-12-29T18:16:35.927510+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK xavfsizlik maqsadi - ETSI EN 319 401 Alignment

| Maydon | Qiymat |
|-------|-------|
| Hujjat versiyasi | 0,1 (2026-02-12) |
| Qo'llash doirasi | Android SDK (`java/iroha_android/` ostida mijoz kutubxonalari va qoʻllab-quvvatlovchi skriptlar/hujjatlar) |
| Egasi | Muvofiqlik va qonunchilik (Sofiya Martins) |
| Taqrizchilar | Android dasturi rahbari, reliz muhandisligi, SRE boshqaruvi |

## 1. BO tavsifi

Baholash maqsadi (TOE) Android SDK kutubxona kodi (`java/iroha_android/src/main/java`), uning konfiguratsiya yuzasi (`ClientConfig` + Norito qabul qilish) va `roadmap.md` uchun `roadmap.md` uchun havola qilingan operatsion asboblardan iborat.

Asosiy komponentlar:

1. **Konfiguratsiyani qabul qilish** — yaratilgan `iroha_config` manifestidan `ClientConfig` iplari Torii so‘nggi nuqtalari, TLS siyosatlari, qayta urinishlar va telemetriya ilgaklari (I1800100-dan keyingi ishga tushirilgandan keyin o‘zgarmaslikni ta’minlaydi).
2. **Kalitlarni boshqarish / StrongBox** — Uskuna bilan qoʻllab-quvvatlangan imzolash `docs/source/sdk/android/key_management.md` da hujjatlashtirilgan siyosatlar bilan `SystemAndroidKeystoreBackend` va `AttestationVerifier` orqali amalga oshiriladi. Attestatsiyani olish/tasdiqlash `scripts/android_keystore_attestation.sh` va `scripts/android_strongbox_attestation_ci.sh` CI yordamchisidan foydalanadi.
3. **Telemetriya va redaktsiya** — `docs/source/sdk/android/telemetry_redaction.md` da tasvirlangan umumiy sxema boʻyicha asboblar hunilari, xeshlangan vakolatlarni, chelaklangan qurilma profillarini eksport qiladi va Yordam oʻqish kitobi tomonidan qoʻllaniladigan audit ilgaklarini bekor qiladi.
4. **Operations runbooks** — `docs/source/android_runbook.md` (operator javobi) va `docs/source/android_support_playbook.md` (SLA + eskalatsiya) deterministik bekor qilish, tartibsizlik mashqlari va dalillarni to'plash bilan BOning operatsion izini mustahkamlaydi.
5. **Chiqarish manbasi** — Gradle asosidagi tuzilmalar `docs/source/sdk/android/developer_experience_plan.md` va AND6 muvofiqligini tekshirish roʻyxatida tasvirlangan CycloneDX plaginidan va qayta tiklanadigan tuzilma bayroqlaridan foydalanadi. Chiqarish artefaktlari imzolangan va `docs/source/release/provenance/android/` da o'zaro havola qilingan.

## 2. Aktivlar va taxminlar

| Aktiv | Tavsif | Xavfsizlik maqsadi |
|-------|-------------|--------------------|
| Konfiguratsiya manifestlari | Ilovalar bilan tarqatilgan Norito-dan olingan `ClientConfig` suratlari. | Dam olishda haqiqiylik, yaxlitlik va maxfiylik. |
| Imzolash kalitlari | StrongBox/TEE provayderlari orqali yaratilgan yoki import qilingan kalitlar. | StrongBox afzalligi, attestatsiya jurnali, kalit eksporti yo'q. |
| Telemetriya oqimlari | SDK asboblaridan eksport qilingan OTLP izlari/jurnallari/metrikalari. | Psevdonimizatsiya (hashed vakolatlari), minimallashtirilgan PII, auditni bekor qilish. |
| Ledger o'zaro ta'siri | Norito foydali yuklar, kirish metama'lumotlari, Torii tarmoq trafigi. | O'zaro autentifikatsiya, takrorlashga chidamli so'rovlar, deterministik qayta urinishlar. |

Taxminlar:

- Mobil OT standart sandboxing + SELinux bilan ta'minlaydi; StrongBox qurilmalari Google-ning keymaster interfeysini amalga oshiradi.
- Operatorlar Torii so'nggi nuqtalarini kengash ishonchli CAlar tomonidan imzolangan TLS sertifikatlari bilan ta'minlaydi.
- Mavenga nashr qilishdan oldin infratuzilmani qayta ishlab chiqarish talablariga javob bering.

## 3. Tahdidlar va boshqaruvlar| Tahdid | Nazorat | Dalil |
|--------|---------|----------|
| O'zgartirilgan konfiguratsiya ko'rinishi | `ClientConfig` manifestlarni (xesh + sxema) qo'llashdan oldin tasdiqlaydi va `android.telemetry.config.reload` orqali rad etilgan qayta yuklashni qayd qiladi. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| Imzolash kalitlarining buzilishi | StrongBox talab qiladigan siyosatlar, attestatsiya jabduqlari va qurilma-matritsa tekshiruvlari driftni aniqlaydi; har bir voqea uchun hujjatlashtirilgan bekor qiladi. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| Telemetriyada PII qochqinligi | Blake2b-xeshlangan vakolatlar, chelaklangan qurilma profillari, tashuvchining qoldirilishi, jurnalni bekor qilish. | `docs/source/sdk/android/telemetry_redaction.md`; Qo'llab-quvvatlash o'yin kitobi §8. |
| Torii RPC | da takrorlang yoki o'chirib tashlang `/v1/pipeline` soʻrov tuzuvchisi TLS pinlash, shovqin kanali siyosati va xeshlangan vakolat konteksti bilan qayta urinib koʻring. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (rejalashtirilgan). |
| Imzosiz yoki takrorlanmaydigan nashrlar | AND6 nazorat ro'yxati bilan tasdiqlangan CycloneDX SBOM + Sigstore sertifikatlari; chiqarish RFClari `docs/source/release/provenance/android/` da dalil talab qiladi. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| To'liq bo'lmagan hodisani boshqarish | Runbook + playbook bekor qilish, tartibsizlik mashqlari va eskalatsiya daraxtini belgilaydi; telemetriyani bekor qilish imzolangan Norito so'rovlarini talab qiladi. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. Baholash faoliyati

1. **Dizaynni ko'rib chiqish** — Muvofiqlik + SRE konfiguratsiya, kalitlarni boshqarish, telemetriya va reliz boshqaruvlari ETSI xavfsizlik maqsadlariga mos kelishini tasdiqlaydi.
2. **Amalga kirishni tekshirish** — Avtomatlashtirilgan testlar:
   - `scripts/android_strongbox_attestation_ci.sh` matritsada keltirilgan har bir StrongBox qurilmasi uchun olingan to'plamlarni tekshiradi.
   - `scripts/check_android_samples.sh` va boshqariladigan qurilma CI namuna ilovalari `ClientConfig`/telemetriya shartnomalariga mos kelishini taʼminlaydi.
3. **Operatsion tekshiruv** — `docs/source/sdk/android/telemetry_chaos_checklist.md` bo‘yicha choraklik tartibsizlik mashqlari (qayta tiklash + bekor qilish mashqlari).
4. **Dalillarni saqlash** — `docs/source/compliance/android/` (ushbu jild) ostida saqlangan va `status.md` dan havola qilingan artefaktlar.

## 5. ETSI EN 319 401 Xaritalash| EN 319 401-modda | SDK boshqaruvi |
|------------------|-------------|
| 7.1 Xavfsizlik siyosati | Ushbu xavfsizlik maqsadi + Yordam kitobida hujjatlashtirilgan. |
| 7.2 Tashkiliy xavfsizlik | RACI + qo‘ng‘iroq bo‘yicha egalik. Yordam kitobi §2. |
| 7.3 Aktivlarni boshqarish | Yuqoridagi §2da belgilangan konfiguratsiya, kalit va telemetriya obyekti maqsadlari. |
| 7.4 Kirishni boshqarish | StrongBox siyosatlari + imzolangan Norito artefaktlarini talab qiluvchi ish jarayonini bekor qilish. |
| 7.5 Kriptografik boshqaruv elementlari | AND2 dan kalitlarni yaratish, saqlash va sertifikatlash talablari (kalitlarni boshqarish bo'yicha qo'llanma). |
| 7.6 Operatsiya xavfsizligi | Telemetriya xashing, tartibsizlik mashqlari, hodisalarga javob berish va dalillarni chiqarish. |
| 7.7 Aloqa xavfsizligi | `/v1/pipeline` TLS siyosati + xeshlangan vakolatlar (telemetrik redaktsiya hujjati). |
| 7.8 Tizimni sotib olish / ishlab chiqish | AND5/AND6 rejalarida takrorlanadigan Gradle konstruksiyalari, SBOMlar va kelib chiqish eshiklari. |
| 7.9 Yetkazib beruvchi munosabatlari | Buildkite + Sigstore sertifikatlari uchinchi tomonga qaramlik SBOMlari bilan birga qayd etilgan. |
| 7.10 Hodisalarni boshqarish | Runbook/Playbook eskalatsiyasi, jurnalni bekor qilish, telemetriya xatolik hisoblagichlari. |

## 6. Xizmat

- SDK yangi kriptografik algoritmlarni, telemetriya toifalarini joriy qilganda yoki avtomatlashtirish oʻzgarishlarini chiqarganda ushbu hujjatni yangilang.
- Imzolangan nusxalarni `docs/source/compliance/android/evidence_log.csv` formatida SHA-256 dayjestlari va sharhlovchining imzosi bilan bog'lash.