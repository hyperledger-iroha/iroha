---
lang: az
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# Nazirlik Qırmızı Komanda Vəziyyəti

Bu səhifə [Moderasiya Qırmızı Komanda Planını](moderation_red_team_plan.md) tamamlayır
yaxın müddətli məşq təqvimini, sübut paketlərini və düzəlişləri izləməklə
status. Tutulan artefaktların yanında hər qaçışdan sonra onu yeniləyin
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## Gələcək Təlimlər

| Tarix (UTC) | Ssenari | Sahib(lər) | Sübut Hazırlığı | Qeydlər |
|------------|---------|---------|---------------|-------|
| 2026-11-12 | **Blindfold Əməliyyatı** — Şluzun aşağı salınması cəhdləri ilə Taikai qarışıq rejimli qaçaqmalçılıq məşqi | Təhlükəsizlik Mühəndisliyi (Miyu Sato), Nazirlik Əməliyyatları (Liam O'Connor) | `scripts/ministry/scaffold_red_team_drill.py` paketi `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + quruluş kataloqu `artifacts/ministry/red-team/2026-11/operation-blindfold/` | GAR/Taikai məşğələləri üst-üstə düşür və DNS-in dəyişdirilməsi; başlamazdan əvvəl denilist Merkle snapshot və tablosuna çəkildikdən sonra `export_red_team_evidence.py` işləməsini tələb edir. |

## Son Drill Snapshot

| Tarix (UTC) | Ssenari | Evidence Bundle | Təmir və Təqiblər |
|------------|---------|-----------------|--------------------------|
| 2026-08-18 | **SeaGlass Əməliyyatı** — Gateway qaçaqmalçılığı, idarəetmənin təkrarı və xəbərdarlığın məşqi | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana ixracı, Alertmanager qeydləri, `seaglass_evidence_manifest.json`) | **Açıq:** replay möhürünün avtomatlaşdırılması (`MINFO-RT-17`, sahib: Governance Ops, 2026-09-05 tarixinə qədər); pin idarə paneli SoraFS (`MINFO-RT-18`, Müşahidə oluna bilər, 2026-08-25 tarixində) kimi dondurulur. **Qapalı:** jurnal şablonu Norito manifest heşlərini daşımaq üçün yeniləndi. |

## İzləmə və Alətlər

- Enjeksiyon üçün qablaşdırma üçün `scripts/ministry/moderation_payload_tool.py` istifadə edin
  hər ssenari üzrə faydalı yüklər və rədd edilmiş yamaqlar.
- `scripts/ministry/export_red_team_evidence.py` vasitəsilə tablosuna / qeydlərə yazın
  hər məşqdən dərhal sonra sübut manifestində imzalanmış hash var.
- CI mühafizəsi `ci/check_ministry_red_team.sh` məşq hesabatlarını yerinə yetirir
  yertutan mətni ehtiva etmir və istinad edilən artefaktlar əvvəllər mövcuddur
  birləşmə.

İstinad edilən canlı xülasə üçün baxın `status.md` (§ *Nazirliyin qırmızı komanda statusu*)
həftəlik koordinasiya çağırışlarında.