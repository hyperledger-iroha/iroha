---
lang: az
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS Təlim İş Kitabı Şablonu

Bu iş kitabını hər bir təlim qrupu üçün kanonik vəsait kimi istifadə edin. Əvəz edin
İştirakçılara paylamadan əvvəl yer tutucuları (`<...>`).

## Sessiya təfərrüatları
- Suffiks: `<.sora | .nexus | .dao>`
- Döngü: `<YYYY-MM>`
- Dil: `<ar/es/fr/ja/pt/ru/ur>`
- Fasilitator: `<name>`

## Laboratoriya 1 — KPI ixracı
1. Portalın KPI idarə panelini açın (`docs/portal/docs/sns/kpi-dashboard.md`).
2. `<suffix>` şəkilçisi və `<window>` vaxt diapazonu ilə süzün.
3. PDF + CSV snapşotlarını ixrac edin.
4. İxrac edilmiş JSON/PDF-nin SHA-256-nı burada qeyd edin: `______________________`.

## Laboratoriya 2 — Manifest təlimi
1. `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`-dən nümunə manifestini götürün.
2. `cargo run --bin sns_manifest_check -- --input <file>` ilə təsdiqləyin.
3. `scripts/sns_zonefile_skeleton.py` ilə həlledici skelet yaradın.
4. Fərq xülasəsini yapışdırın:
   ```
   <git diff output>
   ```

## Laboratoriya 3 — Mübahisə simulyasiyası
1. Dondurmağa başlamaq üçün qəyyum CLI-dən istifadə edin (hal id `<case-id>`).
2. Mübahisə hashini qeyd edin: `______________________`.
3. Sübut jurnalını `artifacts/sns/training/<suffix>/<cycle>/logs/`-ə yükləyin.

## Laboratoriya 4 — Əlavənin avtomatlaşdırılması
1. Grafana idarə panelini JSON ixrac edin və onu `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`-ə kopyalayın.
2. Çalışın:
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
3. Əlavə yolunu + SHA-256 çıxışını yapışdırın: `________________________________`.

## Əlaqə qeydləri
- Nə aydın deyildi?
- Zamanla hansı laboratoriyalar fəaliyyət göstərib?
- Alət səhvləri müşahidə edildi?

Tamamlanmış iş dəftərlərini fasilitatora qaytarın; altındadırlar
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.