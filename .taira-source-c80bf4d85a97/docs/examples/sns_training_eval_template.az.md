---
lang: az
direction: ltr
source: docs/examples/sns_training_eval_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0397eec8373494a74060a7244a7f923714723e5feacc4a54374ee4461f0ddde1
source_last_modified: "2025-12-29T18:16:35.078130+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS Təlim Qiymətləndirmə Şablonu

Zəhmət olmasa bu sorğunu hər seansdan sonra dərhal paylayın. Cavablar ola bilər
forma aləti və ya Markdown vasitəsilə çəkilir və altında arxivləşdirilir
`artifacts/sns/training/<suffix>/<cycle>/feedback/`.

## Sessiya metadatası
- şəkilçi:
- Döngü:
- Dil:
- Tarix:
- Fasilitator(lar):

## Qiymətləndirmə şkalası
1 — Zəif · 2 — Əla · 3 — Yaxşı · 4 — Çox yaxşı · 5 — Əla

| Sual | 1 | 2 | 3 | 4 | 5 |
|----------|---|---|---|---|---|
| KPI keçidinin aydınlığı | ☐ | ☐ | ☐ | ☐ | ☐ |
| Laboratoriyaların faydalılığı | ☐ | ☐ | ☐ | ☐ | ☐ |
| Temp + vaxt bölgüsü | ☐ | ☐ | ☐ | ☐ | ☐ |
| Lokallaşdırma keyfiyyəti (slaydlar + asanlaşdırma) | ☐ | ☐ | ☐ | ☐ | ☐ |
| Suffiksin işə salınmasına daxil olan ümumi etimad | ☐ | ☐ | ☐ | ☐ | ☐ |

## Açıq suallar
1. Hansı mövzu daha çox dərinliyə ehtiyac duyur?
2. İş kitabında hər hansı alətlər/sənədlər çatışmırdı?
3. Lokalizasiya gözləntilərinizə cavab verdimi? Əgər yoxsa, niyə?
4. Proqramın izləməli olduğu əlavə şərhlər / blokerlər.

## Təqib
- `[]` Rəy idarəetmə izləyicisinə daxil olub (bilet: __________)
- `[]` Əlavənin ixracı çəkildi (yol: ____________________)
- `[]` Təyin edilmiş fəaliyyət elementləri (sahibi + son tarix)