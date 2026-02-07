---
lang: uz
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS trening ish kitobi shabloni

Ushbu ish kitobidan har bir o'quv guruhi uchun kanonik tarqatma material sifatida foydalaning. O'zgartiring
ishtirokchilarga tarqatishdan oldin joy egalari (`<...>`).

## Seans tafsilotlari
- Suffiks: `<.sora | .nexus | .dao>`
- Tsikl: `<YYYY-MM>`
- Til: `<ar/es/fr/ja/pt/ru/ur>`
- Fasilitator: `<name>`

## Laboratoriya 1 - KPI eksporti
1. Portal KPI asboblar panelini oching (`docs/portal/docs/sns/kpi-dashboard.md`).
2. `<suffix>` qo'shimchasi va `<window>` vaqt oralig'i bo'yicha filtrlang.
3. PDF + CSV snapshotlarini eksport qiling.
4. Eksport qilingan JSON/PDF ning SHA-256-ni bu yerda yozing: `______________________`.

## Laboratoriya 2 - Manifest matkapi
1. `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` dan manifest namunasini oling.
2. `cargo run --bin sns_manifest_check -- --input <file>` bilan tasdiqlang.
3. `scripts/sns_zonefile_skeleton.py` bilan rezolyutsiya skeletini yarating.
4. Farq xulosasini joylashtiring:
   ```
   <git diff output>
   ```

## 3-laboratoriya — bahslarni simulyatsiya qilish
1. Muzlatishni boshlash uchun qo'riqchi CLI dan foydalaning (xolat identifikatori `<case-id>`).
2. Bahs xeshini yozib oling: `______________________`.
3. Dalillar jurnalini `artifacts/sns/training/<suffix>/<cycle>/logs/` ga yuklang.

## Laboratoriya 4 - Ilovani avtomatlashtirish
1. Grafana asboblar panelini JSON eksport qiling va uni `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` ga nusxalang.
2. Yugurish:
   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Ilova yo'lini + SHA-256 chiqishini joylashtiring: `________________________________`.

## Fikr-mulohazalar
- Nima tushunarsiz edi?
- Vaqt o'tishi bilan qaysi laboratoriyalar ishlagan?
- Asboblarni ishlatishda xatolar kuzatildimi?

Tugallangan ish daftarlarini fasilitatorga qaytaring; ostiga tegishlidirlar
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.