---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ae38d9ff7f10a14e63f6d47490dbbe56c9d3b207a30a5899e63414cb726a88f7
source_last_modified: "2026-01-05T09:28:11.880522+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Migration Ledger"
description: "Canonical change log tracking every migration milestone, owners, and required follow-ups."
translator: machine-google-reviewed
---

> [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md)-dən uyğunlaşdırılıb.

# SoraFS Miqrasiya Kitabı

Bu kitab SoraFS-də qeydə alınan miqrasiya dəyişikliyi jurnalını əks etdirir.
Memarlıq RFC. Girişlər mərhələyə görə qruplaşdırılır və təsirli olanları sadalayır
pəncərə, təsirə məruz qalan komandalar və tələb olunan tədbirlər. Miqrasiya planına yeniliklər
Həm bu səhifəni, həm də RFC-ni dəyişdirməlisiniz (`docs/source/sorafs_architecture_rfc.md`)
aşağı axın istehlakçılarını uyğunlaşdırmaq üçün.

| Mərhələ | Effektiv Pəncərə | Xülasə dəyişikliyi | Təsirə məruz qalan komandalar | Fəaliyyət maddələri | Status |
|----------|------------------|----------------|----------------|--------------|--------|
| M1 | 7–12-ci həftələr | CI deterministik qurğuları tətbiq edir; səhnələşdirmədə mövcud olan ləqəb sübutları; alətlər açıq gözlənti bayraqlarını ifşa edir. | Sənədlər, Yaddaş, İdarəetmə | Qurğuların imzalanmış qalmasını təmin edin, səhnələşdirmə reyestrində ləqəbləri qeyd edin, buraxılış yoxlama siyahılarını `--car-digest/--root-cid` tətbiqi ilə yeniləyin. | ⏳ Gözləyir |

Rəhbərlik nəzarət təyyarəsi bu mərhələlərə istinad edən protokollar altında yaşayır
`docs/source/sorafs/`. Komandalar hər cərgənin altına tarixli güllə nöqtələri əlavə etməlidirlər
diqqətəlayiq hadisələr baş verdikdə (məsələn, yeni ləqəb qeydiyyatları, reyestr hadisəsi
retrospektivlər) yoxlanıla bilən kağız izi təmin etmək.

## Son Yeniləmələr

- 2025-11-01 - İdarəetmə şurasına `migration_roadmap.md` tirajlandı və
  baxılmaq üçün operator siyahıları; Şuranın növbəti sessiyasında imzalanmasını gözləyir
  (istin: `docs/source/sorafs/council_minutes_2025-10-29.md` təqibi).
- 2025-11-02 — Pin Reyestrinin qeydiyyatı ISI indi paylaşılan chunker/siyasəti tətbiq edir
  `sorafs_manifest` köməkçiləri vasitəsilə doğrulama, zəncirvari yolları hizalı saxlamaq
  Torii çekləri ilə.
- 2026-02-13 — Mühasibat kitabçasına provayderin reklamını təqdim etmə mərhələləri (R0–R3) əlavə edildi
  əlaqəli tablosunu və operator təlimatını dərc etdi
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).