---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
---

> Kanonik mənbə: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Status: **Beta / ACK-ları gözləyir** (Şəbəkə, Yaddaş, Sənəd rəhbərləri).

## Baxış

Mart snapshot sənəd/məzmun şəbəkəsi təşəbbüslərini ilə uyğunlaşdırılmış şəkildə saxlayır
SoraFS çatdırılma yolları (SF‑3, SF‑6b, SF‑9). Bütün rəhbərlər bunu qəbul etdikdən sonra
Nexus sükan kanalında snapshot, yuxarıdakı "Beta" qeydini çıxarın.

### Mövzulara diqqət yetirin

1. **Prioritet snapşotunu yayımlayın** — təşəkkürləri toplayın və onları daxil edin
   03.05.2025 Şuranın protokolu.
2. **Gateway/DNS start off-out** — yeni asanlaşdırma dəstini məşq edin (Bölmə 6)
   runbook-da) 2025-03-03 seminarından əvvəl.
3. **Operator runbook miqrasiyası** — `Runbook Index` portalı canlıdır; beta-nı ifşa edin
   rəyçinin qeydiyyatdan keçməsindən sonra URL-i önizləyin.
4. **SoraFS çatdırılma ipləri** — SF‑3/6b/9 qalan işləri plan/yol xəritəsi ilə uyğunlaşdırın:
   - `sorafs-node` PoR qəbulu işçisi + statusun son nöqtəsi.
   - Rust/JS/Swift orkestr inteqrasiyaları üzrə CLI/SDK bağlayıcı cilası.
   - PoR koordinatorunun iş vaxtı naqilləri və GovernanceLog hadisələri.

Tam cədvəl, paylama yoxlama siyahısı və jurnal qeydləri üçün mənbə faylına baxın.