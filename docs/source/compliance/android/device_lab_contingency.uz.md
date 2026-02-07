---
lang: uz
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2025-12-29T18:16:35.923121+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Qurilma laboratoriyasining favqulodda holatlar jurnali

Bu yerda Android qurilma-laboratoriya favqulodda vaziyat rejasining har bir faollashuvini yozib oling.
Muvofiqlik tekshiruvlari va kelajakdagi tayyorlik auditlari uchun yetarlicha tafsilotlarni kiriting.

| Sana | Trigger | Amalga oshirilgan harakatlar | Kuzatuvlar | Egasi |
|------|---------|---------------|------------|-------|
| 2026-02-11 | Pixel8 Pro liniyasi uzilishi va Pixel8a yetkazib berish kechiktirilganidan keyin sig‘im 78% ga tushdi (qarang: `android_strongbox_device_matrix.md`). | Pixel7 qatori asosiy CI maqsadiga koʻtarildi, umumiy Pixel6 parki qarzga olindi, chakana hamyon namunasi uchun rejalashtirilgan Firebase Test Lab tutun sinovlari va AND6 rejasi boʻyicha tashqi StrongBox laboratoriyasi jalb qilindi. | Pixel8 Pro uchun nosoz USB-C uyasiga almashtiring (muddati 2026-02-15); Pixel8a kelishi va qayta ishlash quvvati hisobotini tasdiqlang. | Uskuna laboratoriyasi rahbari |
| 2026-02-13 | Pixel8 Pro uyasi almashtirildi va GalaxyS24 tasdiqlandi, sig‘im 85% ga tiklandi. | `pixel8pro-strongbox-a` va `s24-strongbox-a` teglari bilan ikkilamchi, qayta yoqilgan `android-strongbox-attestation` Buildkite ishiga Pixel7 qatori qaytarildi, tayyorlik matritsasi + dalillar jurnali yangilandi. | Pixel8a yetkazib berish muddatini kuzatib boring (hali kutilmoqda); zaxira markaz inventarlarini hujjatlashtirilgan holda saqlang. | Uskuna laboratoriyasi rahbari |