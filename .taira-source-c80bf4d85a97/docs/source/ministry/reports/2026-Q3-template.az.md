---
lang: az
direction: ltr
source: docs/source/ministry/reports/2026-Q3-template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f313d8010f2a7174c90f51dea512bcab6eb4a207df9199f28a7352944cb43c8b
source_last_modified: "2025-12-29T18:16:35.981653+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency Report — 2026 Q3 (Template)
summary: Scaffold for the MINFO-8 quarterly transparency packet; replace all tokens before publication.
quarter: 2026-Q3
translator: machine-google-reviewed
---

<!--
  Usage:
    1. Copy this file when drafting a new quarter (e.g., 2026-Q3 → 2026-Q3.md).
    2. Replace every {{TOKEN}} marker and remove instructional callouts.
    3. Attach supporting artefacts (data appendix, CSVs, manifest, Grafana export) under artifacts/ministry/transparency/<YYYY-Q>/.
-->

# İcra Xülasəsi

> Moderasiyanın dəqiqliyi, müraciətin nəticələri, rədd cavabı və xəzinədarlıq məqamlarının bir paraqrafdan ibarət xülasəsini təqdim edin. Buraxılışın T+14 son tarixinə uyğun olub-olmadığını qeyd edin.

## Rüb Baxılır

### Əsas məqamlar
- {{HIGHLIGHT_1}}
- {{HIGHLIGHT_2}}
- {{HIGHLIGHT_3}}

### Risklər və azaldılması

| Risk | Təsir | Azaldılması | Sahibi | Status |
|------|--------|------------|-------|--------|
| {{RISK_1}} | {{Təsir}} | {{Yunaltma}} | {{Sahibi}} | {{Status}} |
| {{RISK_2}} | {{Təsir}} | {{Yunaltma}} | {{Sahibi}} | {{Status}} |

## Metriklərə İcmal

Bütün ölçülər DP dezinfeksiyaedicisi işlədikdən sonra `ministry_transparency_builder` (Norito paketi) əsasında yaranır. Aşağıda istinad edilən müvafiq CSV dilimlərini əlavə edin.

### AI Moderasiya Dəqiqliyi

| Model Profili | Region | FP Rate (Hədəf) | FN Rate (Hədəf) | Drift vs Kalibrləmə | Nümunə Ölçüsü | Qeydlər |
|-------------|--------|------------------|------------------|----------------------|-------------|-------|
| {{profil}} | {{region}} | {{fp_rate}} ({{fp_target}}) | {{fn_rate}} ({{fn_target}}) | {{drift}} | {{nümunələr}} | {{qeydlər}} |

### Apellyasiya və Panel Fəaliyyəti

| Metrik | Dəyər | SLA Hədəfi | Trend vs Q-1 | Qeydlər |
|--------|-------|------------|--------------|-------|
| Qəbul edilən müraciətlər | {{müraciətlər_qəbul edildi}} | {{sla}} | {{delta}} | {{qeydlər}} |
| Median həlletmə vaxtı | {{median_qətnamə}} | {{sla}} | {{delta}} | {{qeydlər}} |
| Geri dönüş dərəcəsi | {{reversal_rate}} | {{hədəf}} | {{delta}} | {{qeydlər}} |
| Paneldən istifadə | {{panel_istifadəsi}} | {{hədəf}} | {{delta}} | {{qeydlər}} |

### İnkarçı və Təcili Canon

| Metrik | Say | DP Səs-küy (ε) | Fövqəladə Bayraqlar | TTL Uyğunluğu | Qeydlər |
|--------|-------|--------------|-----------------|----------------|-------|
| Hash əlavələri | {{əlavələr}} | {{epsilon_counts}} | {{bayraqlar}} | {{ttl_status}} | {{qeydlər}} |
| Hash çıxarılması | {{çıxarılmalar}} | {{epsilon_counts}} | {{bayraqlar}} | {{ttl_status}} | {{qeydlər}} |
| Canon çağırışları | {{canon_invocations}} | yox | {{bayraqlar}} | {{ttl_status}} | {{qeydlər}} |

### Xəzinədarlıq Hərəkətləri

| Axın | Məbləğ (MINFO) | Mənbəyə istinad | Qeydlər |
|------|----------------|-----------------|-------|
| Əmanətlərə müraciət edin | {{miqdar}} | {{tx_ref}} | {{qeydlər}} |
| Panel mükafatları | {{miqdar}} | {{tx_ref}} | {{qeydlər}} |
| Əməliyyat xərcləri | {{miqdar}} | {{tx_ref}} | {{qeydlər}} |

### Könüllülər və Yardım İşarələri

| Metrik | Dəyər | Hədəf | Qeydlər |
|--------|-------|--------|-------|
| Könüllülərin brifinqləri dərc olunub | {{dəyər}} | {{hədəf}} | {{qeydlər}} |
| Əhatə olunan dillər | {{dəyər}} | {{hədəf}} | {{qeydlər}} |
| İdarəetmə seminarları keçirilib | {{dəyər}} | {{hədəf}} | {{qeydlər}} |

## Diferensial Məxfilik və Sanitizasiya

Dezinfeksiyaedicinin işini yekunlaşdırın və RNG öhdəliyini daxil edin.

- Təmizləyici işi: `{{CI_JOB_URL}}`
- DP parametrləri: ε={{epsilon_total}}, δ={{delta_total}}
- RNG öhdəliyi: `{{blake3_seed_commitment}}`
- Sızdırılan vedrələr: {{suppressed_buckets}}
- QA rəyçisi: {{rəyçi}}

`artifacts/ministry/transparency/{{Quarter}}/dp_report.json` əlavə edin və hər hansı əl müdaxilələrini qeyd edin.## Məlumat Qoşmaları

| Artefakt | Yol | SHA-256 | SoraFS-ə yükləndiniz? | Qeydlər |
|----------|------|---------|---------------------|-------|
| Xülasə PDF | `artifacts/ministry/transparency/{{Quarter}}/summary.pdf` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |
| Norito məlumat əlavəsi | `artifacts/ministry/transparency/{{Quarter}}/data/appendix.norito` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |
| Metrik CSV paketi | `artifacts/ministry/transparency/{{Quarter}}/data/csv/` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |
| Grafana ixrac | `dashboards/grafana/ministry_transparency_overview.json` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |
| Xəbərdarlıq qaydaları | `dashboards/alerts/ministry_transparency_rules.yml` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |
| Mənşə manifest | `artifacts/ministry/transparency/{{Quarter}}/manifest.json` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |
| Manifest imzası | `artifacts/ministry/transparency/{{Quarter}}/manifest.json.sig` | {{hash}} | {{Bəli/Xeyr}} | {{qeydlər}} |

## Nəşr metadatası

| Sahə | Dəyər |
|-------|-------|
| Buraxılış rübü | {{Rüb}} |
| Buraxılış vaxt damğası (UTC) | {{zaman damgası}} |
| SoraFS CID | `{{cid}}` |
| İdarəetmə səs nömrəsi | {{vote_id}} |
| Manifest həzm (`blake2b`) | `{{manifest_digest}}` |
| Git commit / tag | `{{git_rev}}` |
| Sahibini buraxın | {{sahibi}} |

## Təsdiqlər

| Rol | Adı | Qərar | Vaxt möhürü | Qeydlər |
|------|------|----------|-----------|-------|
| Nazirliyin Müşahidə Edilməsi TL | {{name}} | ✅/⚠️ | {{zaman damgası}} | {{qeydlər}} |
| İdarəetmə Şurası ilə Əlaqə | {{name}} | ✅/⚠️ | {{zaman damgası}} | {{qeydlər}} |
| Sənədlər/Comms Rəhbəri | {{name}} | ✅/⚠️ | {{zaman damgası}} | {{qeydlər}} |

## Dəyişikliklər və İzləmələr

- {{CHANGELOG_ITEM_1}}
- {{CHANGELOG_ITEM_2}}

### Fəaliyyət elementlərini açın

| Maddə | Sahibi | Vaxtı | Status | Qeydlər |
|------|-------|-----|--------|-------|
| {{Fəaliyyət}} | {{Sahibi}} | {{Müddət}} | {{Status}} | {{Qeydlər}} |

### Əlaqə

- Əsas əlaqə: {{contact_name}} (`{{chat_handle}}`)
- Eskalasiya yolu: {{escalation_details}}
- Paylanma siyahısı: {{mailing_list}}

_Şablon versiyası: 2026-03-25. Struktur dəyişiklikləri edərkən təftiş tarixini yeniləyin._