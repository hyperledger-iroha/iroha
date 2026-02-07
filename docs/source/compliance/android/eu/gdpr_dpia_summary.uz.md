---
lang: uz
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2025-12-29T18:16:35.925474+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GDPR DPIA xulosasi — Android SDK telemetriyasi (AND7)

| Maydon | Qiymat |
|-------|-------|
| Baholash sanasi | 2026-02-12 |
| Qayta ishlash faoliyati | Android SDK telemetriyasini umumiy OTLP backendlariga eksport qilish |
| Kontrollerlar / Protsessorlar | SORA Nexus Ops (nazoratchi), hamkor operatorlar (qo'shma kontrollerlar), Hyperledger Iroha Contributors (protsessor) |
| Malumot hujjatlari | `docs/source/sdk/android/telemetry_redaction.md`, `docs/source/android_support_playbook.md`, `docs/source/android_runbook.md` |

## 1. Qayta ishlash tavsifi

- **Maqsad:** Rust tugun sxemasini aks ettirganda (`telemetry_redaction.md` 2-bandi) AND7 kuzatilishini (kechikish, qayta urinishlar, attestatsiya holati) qo‘llab-quvvatlash uchun zarur bo‘lgan operativ telemetriyani ta’minlash.
- **Tizimlar:** Android SDK asboblari -> OTLP eksportchisi -> SRE tomonidan boshqariladigan umumiy telemetriya kollektori (Qoʻllab-quvvatlash oʻquv kitobi §8 ga qarang).
- **Ma'lumotlar sub'ektlari:** Android SDK-ga asoslangan ilovalardan foydalanadigan operator xodimlari; quyi oqim Torii so'nggi nuqtalari (vakolat qatorlari telemetriya siyosati bo'yicha xeshlangan).

## 2. Ma'lumotlarni inventarizatsiya qilish va kamaytirish choralari

| Kanal | Maydonlar | PII xavfi | Yumshatish | Saqlash |
|---------|--------|----------|------------|-----------|
| Izlar (`android.torii.http.request`, `android.torii.http.retry`) | Marshrut, holat, kechikish | Past (PII yo'q) | Vakolat Blake2b + aylanuvchi tuz bilan xeshlangan; hech qanday foydali yuk jismlari eksport qilinmagan. | 7-30 kun (har bir telemetriya hujjati uchun). |
| Voqealar (`android.keystore.attestation.result`) | Taxallus yorlig'i, xavfsizlik darajasi, attestatsiya dayjesti | O'rta (operativ ma'lumotlar) | Taxallus (`alias_label`), `actor_role_masked`, Norito audit tokenlari bilan bekor qilish uchun qayd etilgan. | Muvaffaqiyat uchun 90 kun, bekor qilish/muvaffaqiyatsizlik uchun 365 kun. |
| Ko'rsatkichlar (`android.pending_queue.depth`, `android.telemetry.export.status`) | Navbatlar soni, eksportchi holati | Past | Faqat jamlangan hisoblar. | 90 kun. |
| Qurilma profil o'lchagichlari (`android.telemetry.device_profile`) | SDK asosiy, apparat darajasi | O'rta | Paqir (emulator/iste'molchi/korxona), OEM yoki seriya raqamlari yo'q. | 30 kun. |
| Tarmoq kontekstidagi hodisalar (`android.telemetry.network_context`) | Tarmoq turi, rouming bayrog'i | O'rta | Operator nomi butunlay olib tashlandi; abonent ma'lumotlaridan qochish uchun muvofiqlik talabini qo'llab-quvvatlaydi. | 7 kun. |

## 3. Qonuniy asoslar va huquqlar

- **Qonuniy asos:** Qonuniy manfaat (6(1)(f)-modda) — tartibga solinadigan buxgalteriya hisobi mijozlarining ishonchli ishlashini ta'minlash.
- ** Zarurlik testi:** Operatsion sog'liq bilan cheklangan ko'rsatkichlar (foydalanuvchi kontenti yo'q); xeshlangan avtoritet Rust tugunlari bilan tenglikni faqat vakolatli qo'llab-quvvatlash xodimlariga (ish oqimini bekor qilish orqali) qayta tiklanadigan xaritalash orqali ta'minlaydi.
- **Muvozanat testi:** Telemetriya oxirgi foydalanuvchi ma'lumotlariga emas, balki operator tomonidan boshqariladigan qurilmalarga mo'ljallangan. Bekor qilish uchun Norito imzosi qoʻyilgan artefaktlar qoʻllab-quvvatlash + Muvofiqlik tomonidan koʻrib chiqiladi (Yordam kitobi §3 + §9).
- **Maʼlumotlar obyekti huquqlari:** Operatorlar telemetriyani eksport/oʻchirishni soʻrash uchun Support Engineering (oʻyin kitobi §2) bilan bogʻlanishadi. Redaktsiyani bekor qilish va jurnallar (telemetriya hujjati §Signal Inventory) 30 kun ichida bajarilishini ta'minlaydi.

## 4. Xavfni baholash| Xavf | Ehtimollik | Ta'sir | Qoldiq yumshatish |
|------|------------|--------|---------------------|
| Hashed organlari orqali qayta identifikatsiya | Past | O'rta | `android.telemetry.redaction.salt_version` orqali qayd etilgan tuz aylanishi; xavfsiz omborda saqlanadigan tuzlar; har chorakda tekshiriladi. |
| Profil chelaklari orqali qurilma barmoq izlari | Past | O'rta | Faqat darajali + SDK asosiy eksport qilinadi; Yordam Playbook OEM/seriyali ma'lumotlar uchun eskalatsiya so'rovlarini taqiqlaydi. |
| Noto'g'ri foydalanishni bekor qilish PII | Juda past | Yuqori | Norito bekor qilish soʻrovlari jurnalga kiritilgan, muddati 24 soat ichida tugaydi, SRE tasdiqlanishi kerak (`docs/source/android_runbook.md` §3). |
| Yevropa Ittifoqidan tashqarida telemetriya saqlash | O'rta | O'rta | YI + JP mintaqalarida joylashtirilgan kollektorlar; OTLP backend konfiguratsiyasi orqali amalga oshirilgan saqlash siyosati (Yordam kitobi §8 da hujjatlashtirilgan). |

Yuqoridagi nazorat va doimiy monitoringni hisobga olgan holda qoldiq xavf maqbul deb hisoblanadi.

## 5. Harakatlar va kuzatuvlar

1. **Har chorakda ko'rib chiqish:** Telemetriya sxemalarini, tuz aylanishlarini va jurnallarni bekor qilishni tasdiqlash; hujjat `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`.
2. **Kross-SDK moslashuvi:** Barqaror xeshlash/paqirlash qoidalarini saqlash uchun Swift/JS ta’minotchilari bilan muvofiqlashing (AND7 yo‘l xaritasida kuzatilgan).
3. **Hamkor xabarlari:** DPIA xulosasini hamkorlarni ishga tushirish to‘plamlariga qo‘shing (Yordam kitobi §9) va `status.md` dan ushbu hujjatga havola qiling.