---
lang: az
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Təsirin Qiymətləndirilməsi Alətləri (MINFO‑4b)

Yol xəritəsi arayışı: **MINFO‑4b — Təsirin qiymətləndirilməsi aləti.**  
Sahib: İdarəetmə Şurası / Analitika

Bu qeyd indi `cargo xtask ministry-agenda impact` əmrini sənədləşdirir
referendum paketləri üçün tələb olunan avtomatlaşdırılmış hash-ailə fərqini yaradır. The
alət təsdiqlənmiş Gündəlik Şurasının təkliflərini, dublikat reyestri və istehlak edir
isteğe bağlı rədd siyahısı/siyasət snapshot, belə ki, rəyçilər dəqiq hansını görə bilsinlər
barmaq izləri yenidir, mövcud siyasətlə toqquşur və neçə giriş
hər bir hash ailəsi öz töhfəsini verir.

## Girişlər

1. **Gündəm təklifləri.** Sonrakı bir və ya daha çox fayl
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Onları açıq şəkildə `--proposal <path>` ilə ötürün və ya əmri a-a yönəldin
   `--proposal-dir <dir>` və bu yolun altındakı hər `*.json` faylı vasitəsilə kataloq
   daxildir.
2. **Dublikat reyestr (istəyə görə).** Uyğun olan JSON faylı
   `docs/examples/ministry/agenda_duplicate_registry.json`. Münaqişələr var
   `source = "duplicate_registry"` altında bildirildi.
3. **Siyasət snapşotu (istəyə görə).** Hər şeyi sadalayan yüngül manifest
   barmaq izi artıq GAR/Nazirlik siyasəti ilə tətbiq edilib. Yükləyici gözləyir
   sxem aşağıda göstərilmişdir (bax
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   tam nümunə üçün):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

`hash_family:hash_hex` barmaq izi təklif hədəfinə uyğun gələn hər hansı giriş
istinad edilən `policy_id` ilə `source = "policy_snapshot"` altında məlumat verilmişdir.

## İstifadəsi

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Əlavə təkliflər təkrarlanan `--proposal` bayraqları vasitəsilə və ya
bütün referendum partiyasını ehtiva edən bir kataloq təmin edir:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

`--out` buraxıldıqda komanda yaradılan JSON-u stdout-a çap edir.

## Çıxış

Hesabat imzalanmış artefaktdır (onu referendum paketinin altında qeyd edin
`artifacts/ministry/impact/` kataloqu) aşağıdakı quruluşla:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Bu JSON-u neytral xülasə ilə yanaşı hər referendum faylına əlavə edin
panelistlər, münsiflər və idarəetmə müşahidəçiləri dəqiq partlayış radiusunu görə bilərlər
hər bir təklif. Çıxış deterministikdir (hash ailəsinə görə sıralanır) və təhlükəsizdir
CI/runbooks-a daxil edin; dublikat reyestr və ya siyasət snapshot dəyişirsə,
əmri təkrar yerinə yetirin və səsvermə başlamazdan əvvəl təzələnmiş artefaktı əlavə edin.

> **Növbəti addım:** yaradılan təsir hesabatını daxil edin
> [`cargo xtask ministry-panel packet`](referendum_packet.md) beləliklə
> `ReferendumPacketV1` dosyesində həm hash ailəsinin parçalanması, həm də
> nəzərdən keçirilən təklif üçün ətraflı münaqişə siyahısı.