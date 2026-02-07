---
lang: az
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76fb66d0b0380ea75c7515d3c9b5f73bafc98481f66f28eed05ab5afd3509abf
source_last_modified: "2025-12-29T18:16:35.177356+00:00"
translation_last_reviewed: 2026-02-07
id: training-collateral
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
---

> Güzgülər `docs/source/sns/training_collateral.md`. Brifinq zamanı bu səhifədən istifadə edin
> hər bir şəkilçinin işə salınmasından əvvəl qeydiyyatçı, DNS, qəyyum və maliyyə qrupları.

## 1. Kurikulum snapshot

| Track | Məqsədlər | Öncədən oxumaq |
|-------|------------|-----------|
| Qeydiyyatçı əməliyyatları | Manifestləri təqdim edin, KPI panellərinə nəzarət edin, səhvləri artırın. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS & Gateway | Resolver skeletlərini tətbiq edin, dondurma/geri geri çəkilmələri məşq edin. | `sorafs/gateway-dns-runbook`, birbaşa rejim siyasət nümunələri. |
| Qəyyumlar və şura | Mübahisələri icra edin, idarəetmə əlavələrini yeniləyin, əlavələri qeyd edin. | `sns/governance-playbook`, stüard bal kartları. |
| Maliyyə və analitika | ARPU/toplu ölçüləri götürün, əlavə paketləri dərc edin. | `finance/settlement-iso-mapping`, KPI idarə paneli JSON. |

### Modul axını

1. **M1 — KPI oriyentasiyası (30 dəq):** Gəzinti şəkilçisi filtrləri, ixrac və qaçaq
   sayğacları dondurun. Çatdırılır: SHA-256 həzm ilə PDF/CSV snapshotları.
2. **M2 — Manifestin həyat dövrü (45 dəqiqə):** Qeydiyyatçı manifestlərini qurun və doğrulayın,
   `scripts/sns_zonefile_skeleton.py` vasitəsilə həlledici skeletlər yaradın. Çatdırılma:
   git diff skelet + GAR sübutunu göstərən.
3. **M3 — Mübahisə məşqləri (40 dəq):** Qəyyumun dondurulması + müraciət, tutma simulyasiyası
   mühafizəçi CLI qeydləri `artifacts/sns/training/<suffix>/<cycle>/logs/` altındadır.
4. **M4 — Əlavənin çəkilişi (25 dəqiqə):** İdarə paneli JSON-u ixrac edin və işə salın:

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

   Çatdırılır: yenilənmiş əlavə Markdown + tənzimləyici + portal memo blokları.

## 2. Lokallaşdırma iş axını

- Dillər: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, I180NI05X00.
- Hər bir tərcümə mənbə faylının yanında yaşayır
  (`docs/source/sns/training_collateral.<lang>.md`). `status` + yeniləyin
  Yeniləndikdən sonra `translation_last_reviewed`.
- Dilə görə aktivlər altındadır
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slaydlar/, iş dəftərləri/,
  qeydlər/, jurnallar/).
- İngilis dilini redaktə etdikdən sonra `python3 scripts/sync_docs_i18n.py --lang <code>`-i işə salın
  mənbə ki, tərcüməçilər yeni hashı görsünlər.

### Çatdırılma yoxlama siyahısı

1. Lokallaşdırıldıqdan sonra tərcümə stubunu (`status: complete`) yeniləyin.
2. Slaydları PDF-ə ixrac edin və hər dil üçün `slides/` kataloquna yükləyin.
3. ≤10min KPI gedişini qeyd edin; dil stubundan keçid.
4. Slayd/iş kitabı olan `sns-training` etiketli fayl idarəetmə bileti
   həzmlər, qeyd bağlantıları və əlavə sübutlar.

## 3. Təlim aktivləri

- Slayd kontur: `docs/examples/sns_training_template.md`.
- İş dəftəri şablonu: `docs/examples/sns_training_workbook.md` (hər iştirakçıya bir).
- Dəvət + xatırlatmalar: `docs/examples/sns_training_invite_email.md`.
- Qiymətləndirmə forması: `docs/examples/sns_training_eval_template.md` (cavablar
  `artifacts/sns/training/<suffix>/<cycle>/feedback/` altında arxivləşdirilmişdir).

## 4. Planlaşdırma və ölçülər

| Cycle | Pəncərə | Metriklər | Qeydlər |
|-------|--------|---------|-------|
| 2026-03 | KPI nəzərdən keçirin | Davamiyyət %, əlavə həzm qeyd edildi | `.sora` + `.nexus` kohortlar |
| 2026-06 | Əvvəlcədən `.dao` GA | Maliyyə hazırlığı ≥90% | Siyasət yeniləməsini daxil edin |
| 2026-09 | Genişlənmə | Mübahisəli təlim <20dəq, əlavə SLA ≤2days | SN-7 stimulları ilə uyğunlaşın |

`docs/source/sns/reports/sns_training_feedback.md`-də anonim rəy yazın
belə ki, sonrakı kohortlar lokalizasiyanı və laboratoriyaları təkmilləşdirə bilər.