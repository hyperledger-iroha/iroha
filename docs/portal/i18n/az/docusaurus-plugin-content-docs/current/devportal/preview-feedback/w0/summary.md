---
id: preview-feedback-w0-summary
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W0 midpoint feedback digest
sidebar_label: W0 feedback (midpoint)
description: Midpoint checkpoints, findings, and action items for the core-maintainer preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Maddə | Təfərrüatlar |
| --- | --- |
| Dalğa | W0 — Əsas baxıcılar |
| Dijest tarixi | 2025-03-27 |
| Baxış pəncərəsi | 2025‑03‑25 → 2025‑04‑08 |
| İştirakçılar | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Artefakt etiketi | `preview-2025-03-24` |

## Əsas məqamlar

1. **Yoxlama məbləği iş axını** — Bütün rəyçilər `scripts/preview_verify.sh` təsdiqləyib
   paylaşılan deskriptor/arxiv cütlüyünə qarşı uğur qazandı. Əl ilə ləğv edilmir
   tələb olunur.
2. **Naviqasiya ilə bağlı rəy** — Yan panelin sifarişi ilə bağlı iki kiçik problem qaldırıldı
   (`docs-preview/w0 #1–#2`). Hər ikisi Docs/DevRel-ə yönləndirilir və blok etmir
   dalğa.
3. **SoraFS runbook pariteti** — sorafs-ops-01 daha aydın keçidlər tələb etdi
   `sorafs/orchestrator-ops` və `sorafs/multi-source-rollout` arasında. Təqib
   məsələ qaldırıldı; W1-dən əvvəl müraciət edilməlidir.
4. **Telemetriya baxışı** — müşahidə qabiliyyəti-01 təsdiqlənmiş `docs.preview.integrity`,
   `TryItProxyErrors` və Sınaq proksi qeydləri yaşıl qaldı; heç bir xəbərdarlıq verilmədi.

## Fəaliyyət elementləri

| ID | Təsvir | Sahibi | Status |
| --- | --- | --- | --- |
| W0-A1 | Səthi nəzərdən keçirən sənədlərə (`preview-invite-*` qrupu birlikdə) uyğun olaraq devportal yan panel girişlərini yenidən sıralayın. | Docs-core-01 | ✅ Tamamlandı — yan panel indi nəzərdən keçirən sənədləri ardıcıl olaraq siyahıya alır (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` və `sorafs/multi-source-rollout` arasında açıq keçid əlavə edin. | Sorafs-ops-01 | ✅ Tamamlandı — hər bir runbook indi digərinə keçid verir ki, operatorlar buraxılış zamanı hər iki bələdçini görsünlər. |
| W0-A3 | İdarəetmə izləyicisi ilə telemetriya görüntülərini + sorğu paketini paylaşın. | Müşahidə qabiliyyəti-01 | ✅ Tamamlandı — paket `DOCS-SORA-Preview-W0`-ə əlavə olunub. |

## Çıxış xülasəsi (04-08-2025)

- Beş rəyçinin hamısı tamamlandığını təsdiqlədi, yerli tikililəri təmizlədi və binadan çıxdı
  önizləmə pəncərəsi; `DOCS-SORA-Preview-W0`-də qeydə alınan giriş ləğvləri.
- Dalğa zamanı heç bir insident və ya xəbərdarlıq edilmədi; telemetriya panelləri qaldı
  tam müddət üçün yaşıl.
- Naviqasiya + çarpaz keçid hərəkətləri (W0-A1/A2) həyata keçirilir və əks olunur
  yuxarıdakı sənədlər; telemetriya sübutu (W0-A3) izləyiciyə əlavə olunur.
- Arxivləşdirilmiş sübut paketi: telemetriya skrinşotları, dəvət təşəkkürləri və
  bu həzm izləyici məsələsi ilə əlaqələndirilir.

## Növbəti addımlar

- W1-i açmadan əvvəl W0 fəaliyyət elementlərini həyata keçirin.
- Hüquqi təsdiq və proksi hazırlama yuvası əldə edin, sonra tərəfdaş dalğasını izləyin
  [Önizləmə dəvət axını](../../preview-invite-flow.md)-də qeyd olunan uçuşdan əvvəl addımlar.

_Bu həzm [Önizləmə dəvəti izləyicisindən](../../preview-invite-tracker.md) ilə əlaqələndirilib
DOCS-SORA yol xəritəsini izlənilə bilən saxlayın._