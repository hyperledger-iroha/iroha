---
lang: uz
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2025-12-29T18:16:35.926037+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EI huquqiy ro'yxatdan o'tish memo shabloni

Ushbu eslatmada yo'l xaritasining **AND6** bandi bo'yicha talab qilinadigan huquqiy tekshiruv qayd etilgan
Evropa Ittifoqi (ETSI/GDPR) artefakt paketi tartibga soluvchi organlarga taqdim etiladi. Maslahatchi klonlashi kerak
ushbu shablonni har bir nashr uchun, quyidagi maydonlarni to'ldiring va imzolangan nusxani saqlang
eslatmada keltirilgan o'zgarmas artefaktlar bilan bir qatorda.

## Xulosa

- ** Chiqarish / Poezd:** `<e.g., 2026.1 GA>`
- **Ko'rib chiqish sanasi:** `<YYYY-MM-DD>`
- **Maslahatchi / Sharhlovchi:** `<name + organisation>`
- ** Qo'llash doirasi:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Asosiy chiptalar:** `<governance or legal issue IDs>`

## Artefaktni tekshirish ro'yxati

| Artefakt | SHA-256 | Joylashuv / Havola | Eslatmalar |
|----------|---------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + boshqaruv arxivi | Chiqarish identifikatorlari va tahdid modeli sozlamalarini tasdiqlang. |
| `gdpr_dpia_summary.md` | `<hash>` | Xuddi shu katalog / mahalliylashtirish oynalari | Tahrirlash siyosati havolalari `sdk/android/telemetry_redaction.md` bilan mos kelishiga ishonch hosil qiling. |
| `sbom_attestation.md` | `<hash>` | Dalillar paqiridagi bir xil katalog + kosign to'plami | CycloneDX + kelib chiqish imzolarini tekshiring. |
| Dalillar jurnali qatori | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | Qator raqami `<n>` |
| Qurilma-laboratoriya favqulodda vaziyatlar to'plami | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Ushbu nashrga bog'liq bo'lmagan takroriy takrorlashni tasdiqlaydi. |

> Agar paketda koʻproq fayllar boʻlsa, qoʻshimcha qatorlarni biriktiring (masalan, maxfiylik
> ilovalar yoki DPIA tarjimalari). Har bir artefakt o'zining o'zgarmasligiga murojaat qilishi kerak
> yuklash maqsadi va uni yaratgan Buildkite ishi.

## Topilmalar va istisnolar

- `None.` *(Qolgan xavflarni qamrab oluvchi, kompensatsiya qiluvchi oʻqlar roʻyxati bilan almashtiring
  boshqarish vositalari yoki zarur keyingi harakatlar.)*

## Tasdiqlash

- **Qaror:** `<Approved / Approved with conditions / Blocked>`
- **Imzo / Vaqt tamg'asi:** `<digital signature or email reference>`
- **Kuzatuvchi egalari:** `<team + due date for any conditions>`

Yakuniy eslatmani boshqaruv dalillari paqiriga yuklang, SHA-256-dan nusxa oling
`docs/source/compliance/android/evidence_log.csv` va yuklash yo'lini ulang
`status.md`. Agar qaror "Bloklangan" bo'lsa, AND6 boshqaruviga o'ting
qo'mita va hujjatlarni tuzatish bosqichlari ham yo'l xaritasi issiq ro'yxatida, ham
qurilma-laboratoriya favqulodda vaziyatlar jurnali.