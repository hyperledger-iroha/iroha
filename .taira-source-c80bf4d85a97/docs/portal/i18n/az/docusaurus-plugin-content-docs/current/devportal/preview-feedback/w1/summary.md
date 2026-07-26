---
id: preview-feedback-w1-summary
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner feedback & exit summary
sidebar_label: W1 summary
description: Findings, actions, and exit evidence for the partner/Torii integrator preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Maddə | Təfərrüatlar |
| --- | --- |
| Dalğa | W1 — Partnyorlar və Torii inteqratorları |
| Dəvət pəncərəsi | 2025‑04‑12 → 2025‑04‑26 |
| Artefakt etiketi | `preview-2025-04-12` |
| İzləyici problemi | `DOCS-SORA-Preview-W1` |
| İştirakçılar | sorafs-op-01…03, torii-int-01…02, sdk-partner-01…02, gateway-ops-01 |

## Əsas məqamlar

1. **Yoxlama məbləğinin iş axını** — Bütün rəyçilər `scripts/preview_verify.sh` vasitəsilə deskriptoru/arxivi yoxlayıblar; dəvətnamələrin yanında saxlanılan qeydlər.
2. **Telemetri** — `docs.preview.integrity`, `TryItProxyErrors` və `DocsPortal/GatewayRefusals` idarə panelləri bütün dalğa üçün yaşıl qaldı; heç bir insident və ya xəbərdarlıq səhifələri işə salınmadı.
3. **Sənəd rəyi (`docs-preview/w1`)** — İki kiçik nit təqdim edildi:
   - `docs-preview/w1 #1`: Sınaq bölməsində naviqasiya sözlərini aydınlaşdırın (həll edildi).
   - `docs-preview/w1 #2`: yeniləmə Skrinşotunu yoxlayın (həll olundu).
4. **Runbook pariteti** — SoraFS operatorları `orchestrator-ops` və `multi-source-rollout` arasında yeni çarpaz bağlantıların onların W0 problemlərini həll etdiyini təsdiqlədilər.

## Fəaliyyət elementləri

| ID | Təsvir | Sahibi | Status |
| --- | --- | --- | --- |
| W1-A1 | Yeniləmə `docs-preview/w1 #1`-ə uyğun olaraq naviqasiyanı sınayın. | Docs-core-02 | ✅ Tamamlandı (2025‑04‑18). |
| W1-A2 | Yeniləyin `docs-preview/w1 #2` üçün ekran görüntüsünü sınayın. | Docs-core-03 | ✅ Tamamlandı (2025‑04‑19). |
| W1-A3 | Yol xəritəsində/statusunda tərəfdaş tapıntılarını + telemetriya sübutlarını ümumiləşdirin. | Sənədlər/DevRel aparıcı | ✅ Tamamlandı (bax: tracker + status.md). |

## Çıxış xülasəsi (26-04-2025)

- Səkkiz rəyçinin hamısı son iş saatları ərzində işin tamamlandığını təsdiqlədi, yerli artefaktları təmizlədi və onların girişi ləğv edildi.
- Telemetriya çıxış yolu ilə yaşıl qaldı; son görüntülər `DOCS-SORA-Preview-W1`-ə əlavə edilmişdir.
- Çıxış bildirişləri ilə yenilənmiş dəvət jurnalı; izləyici W1-i 🈴-ə çevirdi və yoxlama nöqtəsi qeydlərini əlavə etdi.
- `artifacts/docs_preview/W1/` altında arxivləşdirilmiş sübut paketi (deskriptor, yoxlama jurnalı, araşdırma çıxışı, Sınaq proksi transkripti, telemetriya skrinşotları, rəy həzmi).

## Növbəti addımlar

- W2 icmasının qəbul planını hazırlayın (idarəetmənin təsdiqi + sorğu şablonu düzəlişləri).
- W2 dalğası üçün önizləmə artefakt etiketini yeniləyin və tarixlər yekunlaşdıqdan sonra uçuşdan əvvəl skripti yenidən işə salın.
- Tətbiq edilə bilən W1 tapıntılarını yol xəritəsi/statusuna daxil edin ki, icma dalğası ən son təlimata malik olsun.