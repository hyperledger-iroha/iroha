---
id: preview-feedback-w2-summary
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W2 community feedback & status
sidebar_label: W2 summary
description: Live digest for the community preview wave (W2).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Maddə | Təfərrüatlar |
| --- | --- |
| Dalğa | W2 — İcma rəyçiləri |
| Dəvət pəncərəsi | 2025‑06‑15 → 2025‑06‑29 |
| Artefakt etiketi | `preview-2025-06-15` |
| İzləyici problemi | `DOCS-SORA-Preview-W2` |
| İştirakçılar | comm-cild-01 … comm-cild-08 |

## Əsas məqamlar

1. **İdarəetmə və alətlər** — İcma qəbulu siyasəti 2025-05-20-də yekdilliklə təsdiq edilmişdir; motivasiya/saat qurşağı sahələri ilə yenilənmiş sorğu şablonu `docs/examples/docs_preview_request_template.md` altında yaşayır.
2. **Uçuşdan əvvəl sübut** — Proksi dəyişikliyini sınayın `OPS-TRYIT-188` 2025‑06‑09 işlədi, Grafana idarə panelləri ələ keçirildi və `preview-2025-06-15` təsviri/yoxlama məbləği/zond çıxışları arşivlendi180NI00.
3. **Dəvət dalğası** — Səkkiz icma rəyçisi 2025‑06‑15-ə dəvət edildi, izləyici dəvət cədvəlinə daxil edilmiş təşəkkürlər; baxışdan əvvəl bütün tamamlanmış yoxlama məbləğinin yoxlanılması.
4. **Əlaqə** — `docs-preview/w2 #1` (alət məsləhəti mətni) və `#2` (lokallaşdırma yan panel sifarişi) 2025-06-18 tarixində təqdim edilib və 2025-06-21 (Sənəd-04-04-) tarixində həll olunub; dalğa zamanı heç bir insident baş verməyib.

## Fəaliyyət elementləri

| ID | Təsvir | Sahibi | Status |
| --- | --- | --- | --- |
| W2-A1 | Ünvan `docs-preview/w2 #1` (alət ipucu yazısı). | Docs-core-04 | ✅ Tamamlanıb 2025‑06‑21 |
| W2-A2 | Ünvan `docs-preview/w2 #2` (lokallaşdırma yan çubuğu). | Docs-core-05 | ✅ Tamamlanıb 2025‑06‑21 |
| W2-A3 | Arxivdən çıxış sübutu + yol xəritəsini/statusunu yeniləyin. | Sənədlər/DevRel aparıcı | ✅ Tamamlanıb 2025‑06‑29 |

## Çıxış xülasəsi (29.06.2025)

- Bütün səkkiz icma rəyçisi tamamlanmanı təsdiqlədi və ilkin baxışa girişi ləğv etdi; izləyici dəvət jurnalında qeydə alınan təşəkkürlər.
- Son telemetriya görüntüləri (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) yaşıl qaldı; logs plus `DOCS-SORA-Preview-W2`-ə əlavə edilmiş proksi transkriptləri sınayın.
- `artifacts/docs_preview/W2/preview-2025-06-15/` altında arxivləşdirilmiş sübut paketi (deskriptor, yoxlama jurnalı, araşdırma çıxışı, keçid hesabatı, Grafana skrinşotları, dəvətnamələr).
- Tracker W2 yoxlama məntəqəsi jurnalı çıxış yolu ilə yeniləndi və W3 planlaşdırması başlamazdan əvvəl yol xəritəsinin yoxlanıla bilən qeydini saxlamasını təmin etdi.