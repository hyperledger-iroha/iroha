---
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-29T18:16:35.107308+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-log
title: W1 feedback & telemetry log
sidebar_label: W1 feedback log
description: Aggregate roster, telemetry checkpoints, and reviewer notes for the first partner preview wave.
translator: machine-google-reviewed
---

Bu jurnal dəvət siyahısı, telemetriya yoxlama məntəqələri və rəyçi rəyini saxlayır
Qəbul tapşırıqlarını müşayiət edən **W1 tərəfdaş önizləməsi**
[`preview-feedback/w1/plan.md`](./plan.md) və dalğa izləyicisi girişi
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Dəvət olanda onu yeniləyin
göndərilir, telemetriya snapshot qeydə alınır və ya rəy elementi triajlanır ki, idarəetmə rəyçiləri təkrar oxuya bilsinlər
xarici biletləri təqib etmədən sübut.

## Kohort siyahısı

| Partnyor ID | Bilet tələb | NDA qəbul | Dəvət göndərildi (UTC) | Ack/ilk giriş (UTC) | Status | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| partnyor-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 04-03-2025 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Tamamlanıb 26.04.2025 | sorafs-op-01; orchestrator doc parite sübutlarına diqqət yetirir. |
| partnyor-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 04-03-2025 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Tamamlanıb 26.04.2025 | sorafs-op-02; təsdiqlənmiş Norito/temetrik keçidlər. |
| partnyor-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04-04-2025 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Tamamlanıb 26.04.2025 | sorafs-op-03; çoxmənbəli əvəzetmə təlimlərini həyata keçirdi. |
| partnyor-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04-04-2025 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Tamamlanıb 26.04.2025 | torii-int-01; Torii `/v2/pipeline` + Kulinariya kitabını nəzərdən keçirin. |
| partnyor-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 04-05-2025 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Tamamlanıb 26.04.2025 | torii-int-02; Cütlənmiş Skrinşot yeniləməsini sınaqdan keçirin (docs-preview/w1 #2). |
| partnyor-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 04-05-2025 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Tamamlanıb 26.04.2025 | sdk-partner-01; JS/Swift yemək kitabı rəyi + ISO körpü ağlını yoxlayır. |
| partnyor-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 11.04.2025 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Tamamlanıb 26.04.2025 | sdk-partner-02; uyğunluq 2025-04-11 tarixində təmizləndi, Connect/telemetri qeydlərinə fokuslanıb. |
| partnyor-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 11.04.2025 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Tamamlanıb 26.04.2025 | Gateway-ops-01; yoxlanılmış şlüz əməliyyat təlimatı + anonim Proksi axınını sınayın. |

Gedən e-poçt göndərilən kimi **Dəvət göndərildi** və **Ack** vaxt ştamplarını doldurun.
Vaxtları W1 planında müəyyən edilmiş UTC cədvəlinə bağlayın.

## Telemetriya nəzarət məntəqələri

| Vaxt möhürü (UTC) | Tablolar / zondlar | Sahibi | Nəticə | Artefakt |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Sənədlər/DevRel + Əməliyyatlar | ✅ Bütün yaşıl | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` transkript | Əməliyyat | ✅ Mərhələli | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`, `probe:portal` | Sənədlər/DevRel + Əməliyyatlar | ✅ Əvvəlcədən dəvət snapshot, heç bir reqressiya | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Yuxarıdakı panellər + Bunu sınayın proxy gecikmə fərqi | Sənədlər/DevRel aparıcı | ✅ Orta nöqtə yoxlanışı keçdi (0 xəbərdarlıq; Bunu sınayın gecikmə p95=410ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26-04-2025 16:25 | Yuxarıdakı idarə panelləri + çıxış zondu | Sənədlər/DevRel + İdarəetmə əlaqəsi | ✅ Snapshotdan çıxın, sıfır əlamətdar xəbərdarlıq | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Gündəlik iş saatı nümunələri (2025-04-13 → 2025-04-25) NDJSON + PNG ixracı kimi paketlənir.
Fayl adları ilə `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
`docs-preview-integrity-<date>.json` və müvafiq ekran görüntüləri.

## Rəy və problem jurnalı

Rəyçi tərəfindən təqdim edilən nəticələri ümumiləşdirmək üçün bu cədvəldən istifadə edin. Hər girişi GitHub/müzakirə ilə əlaqələndirin
bilet plus vasitəsilə ələ strukturlaşdırılmış forma
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| İstinad | Ciddilik | Sahibi | Status | Qeydlər |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Aşağı | Docs-core-02 | ✅ Qərar verilib 2025-04-18 | Aydınlaşdırıldı Bunu cəhd edin naviqasiya mətni + yan panel ankeri (`docs/source/sorafs/tryit.md` yeni etiketlə yeniləndi). |
| `docs-preview/w1 #2` | Aşağı | Docs-core-03 | ✅ Qərar verilib 2025-04-19 | Yeniləndi Sınayın skrinşot + hər rəyçinin sorğusuna görə başlıq; artefakt `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | Məlumat | Sənədlər/DevRel aparıcı | 🢢 Bağlıdır | Qalan şərhlər yalnız sual-cavab idi; `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` altında hər bir tərəfdaşın rəy formasında qeyd olunur. |

## Biliyin yoxlanılması və sorğunun izlənməsi

1. Hər rəyçi üçün viktorina xallarını qeyd edin (hədəf ≥90%); ilə yanaşı ixrac edilmiş CSV-ni əlavə edin
   artefaktları dəvət edin.
2. Əlaqə forması şablonu ilə əldə edilmiş keyfiyyətli sorğu cavablarını toplayın və onları əks etdirin
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` altında.
3. Həddən aşağı bal toplayan hər kəs üçün düzəliş çağırışlarını planlaşdırın və onları bu fayla daxil edin.

Bütün səkkiz rəyçi bilik yoxlamasında ≥94% bal toplayıb (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Təmizləmə zəngləri yoxdur
tələb olunurdu; Hər bir tərəfdaş üçün ixrac anketi altında yaşayır
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Artefakt inventar

- İlkin baxış deskriptoru/yoxlama məbləği paketi: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Prob + keçid-yoxlama xülasəsi: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Proksi dəyişdirmə jurnalını sınayın: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetriya ixracı: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Gündəlik iş saatı telemetriya paketi: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Rəy + sorğu ixracı: rəyçi üçün xüsusi qovluqları altında yerləşdirin
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Bilik yoxlaması CSV və xülasə: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

İnventarizasiyanı izləyici problemi ilə sinxronlaşdırın. Artefaktları kopyalayarkən heşlər əlavə edin
idarəetmə bileti beləliklə, auditorlar qabıq girişi olmadan faylları yoxlaya bilsinlər.