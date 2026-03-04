---
id: preview-feedback-w3-summary
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Maddə | Təfərrüatlar |
| --- | --- |
| Dalğa | W3 — Beta qrupları (maliyyə + əməliyyatlar + SDK tərəfdaşı + ekosistem vəkili) |
| Dəvət pəncərəsi | 2026‑02‑18 → 2026‑02‑28 |
| Artefakt etiketi | `preview-20260218` |
| İzləyici problemi | `DOCS-SORA-Preview-W3` |
| İştirakçılar | maliyyə-beta-01, müşahidə qabiliyyəti-ops-02, partnyor-sdk-03, ekosistem-vəkil-04 |

## Əsas məqamlar

1. **Uçdan uca sübut boru kəməri.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` dalğa başına xülasə yaradır (`artifacts/docs_portal_preview/preview-20260218-summary.json`), həzm edir (`preview-20260218-digest.md`) və `docs/portal/src/data/previewFeedbackSummary.json`-i yeniləyir ki, idarəçiliyin tək rəyçilərə etibar edə bilsin.
2. **Telemetriya + idarəetmə əhatəsi.** Dörd rəyçinin hamısı yoxlama məbləği ilə bağlı girişi qəbul etdi, rəy təqdim etdi və vaxtında ləğv edildi; həzm dalğa zamanı toplanmış Grafana qaçışları ilə yanaşı rəy problemlərinə (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) istinad edir.
3. **Portal səthi.** Yenilənmiş portal cədvəli indi gecikmə və cavab dərəcəsi göstəriciləri ilə qapalı W3 dalğasını göstərir və aşağıdakı yeni jurnal səhifəsi xam JSON jurnalını çəkməyən auditorlar üçün hadisə qrafikini əks etdirir.

## Fəaliyyət elementləri

| ID | Təsvir | Sahibi | Status |
| --- | --- | --- | --- |
| W3-A1 | Önizləmə həzmini çəkin və izləyiciyə əlavə edin. | Sənədlər/DevRel aparıcı | ✅ Tamamlanıb 2026‑02‑28 |
| W3-A2 | Dəvət/sübutları portala + yol xəritəsi/statusuna əks etdirin. | Sənədlər/DevRel aparıcı | ✅ Tamamlanıb 2026‑02‑28 |

## Çıxış xülasəsi (28-02-2026)

- 2026‑02‑18-də göndərilmiş dəvətlər bir neçə dəqiqə sonra daxil edilmiş təşəkkürlər; son telemetriya yoxlanışı keçdikdən sonra 2026-02-28 tarixində ilkin baxışa giriş ləğv edildi.
- Replayability üçün `artifacts/docs_portal_preview/feedback_log.json` tərəfindən lövbərlənmiş xam jurnalla `artifacts/docs_portal_preview/` altında toplanmış Digest + xülasə.
- `DOCS-SORA-Preview-20260218` idarəetmə izləyicisi ilə `docs-preview/20260218` altında təqdim edilmiş problem təqibləri; CSP/Try it qeydləri müşahidə oluna bilən/maliyyə sahiblərinə yönləndirilmiş və həzmdən əlaqələndirilmişdir.
- İzləyici sırası 🈴 Tamamlandı olaraq yeniləndi və portal rəy cədvəli qapalı dalğanı əks etdirir, qalan DOCS-SORA beta-hazırlıq tapşırığını tamamlayır.