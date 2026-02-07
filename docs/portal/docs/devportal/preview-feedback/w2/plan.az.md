---
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9a599a71cc49432334dbf323125756fc6056414a4b8f7622d4cc69edcfbd7503
source_last_modified: "2025-12-29T18:16:35.109243+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w2-plan
title: W2 community intake plan
sidebar_label: W2 plan
description: Intake, approvals, and evidence checklist for the community preview cohort.
translator: machine-google-reviewed
---

| Maddə | Təfərrüatlar |
| --- | --- |
| Dalğa | W2 — İcma rəyçiləri |
| Hədəf pəncərəsi | Q3 2025 week 1 (tentative) |
| Artefakt etiketi (planlaşdırılmış) | `preview-2025-06-15` |
| İzləyici problemi | `DOCS-SORA-Preview-W2` |

## Məqsədlər

1. İcma qəbulu meyarlarını və yoxlama işini müəyyən edin.
2. Təklif olunan siyahı və məqbul istifadəyə dair əlavə üçün idarəetmə təsdiqini əldə edin.
3. Yeni pəncərə üçün yoxlama məbləği ilə təsdiqlənmiş önizləmə artefaktını və telemetriya paketini təzələyin.
4. Dəvət göndərilməzdən əvvəl “Try it proxy” + idarə panellərini səhnələşdirin.

## Tapşırıq bölgüsü

| ID | Tapşırıq | Sahibi | Vaxtı | Status | Qeydlər |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | İcma qəbulu meyarlarının layihəsini (uyğunluq, maksimum yerlər, CoC tələbləri) və idarəetməyə yaymaq | Sənədlər/DevRel aparıcı | 2025-05-15 | ✅ Tamamlandı | Qəbul siyasəti `DOCS-SORA-Preview-W2` ilə birləşdirildi və 2025-05-20 şura iclasında təsdiq edildi. |
| W2-P2 | İcma üçün xüsusi suallarla sorğu şablonunu yeniləyin (motivasiya, mövcudluq, lokalizasiya ehtiyacları) | Docs-core-01 | 2025-05-18 | ✅ Tamamlandı | `docs/examples/docs_preview_request_template.md` indi qəbul formasında istinad edilən İcma bölməsini ehtiva edir. |
| W2-P3 | Qəbul planı üçün təhlükəsiz idarəetmənin təsdiqi (iclas səsverməsi + qeydə alınmış protokollar) | İdarəetmə əlaqəsi | 2025‑05‑22 | ✅ Tamamlandı | Səs 2025-05-20-də yekdilliklə qəbul edildi; dəqiqə + roll zəng `DOCS-SORA-Preview-W2` ilə əlaqələndirilir. |
| W2-P4 | Cədvəl Bunu cəhd edin. Sənədlər/DevRel + Əməliyyatlar | 2025-06-05 | ✅ Tamamlandı | Bileti dəyişdirin `OPS-TRYIT-188` 2025-06-09 02:00-04:00UTC tarixində təsdiqləndi və icra edildi; Grafana skrinşotları biletlə arxivləşdirilmişdir. |
| W2-P5 | Yeni önizləmə artefakt teqi (`preview-2025-06-15`) və arxiv deskriptoru/yoxlama məbləği/prob qeydləri yaradın/doğrulayın | Portal TL | 2025-06-07 | ✅ Tamamlandı | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025‑06‑10 işlədi; `artifacts/docs_preview/W2/preview-2025-06-15/` altında saxlanılan çıxışlar. |
| W2-P6 | İdarəetmə tərəfindən təsdiqlənmiş əlaqə məlumatı ilə icma dəvət siyahısını toplayın (≤25 rəyçi, mərhələli qruplar) | İcma meneceri | 2025-06-10 | ✅ Tamamlandı | 8 icma rəyçisindən ibarət ilk kohort təsdiqləndi; izləyiciyə daxil olan `DOCS-SORA-Preview-REQ-C01…C08` sorğu identifikatorları. |

## Sübut yoxlama siyahısı

- [x] `DOCS-SORA-Preview-W2`-ə əlavə edilmiş idarəetmənin təsdiqi qeydi (görüş qeydləri + səs bağlantısı).
- [x] `docs/examples/` altında tərtib edilmiş yenilənmiş sorğu şablonu.
- [x] `preview-2025-06-15` deskriptoru, yoxlama jurnalı, araşdırma çıxışı, keçid hesabatı və `artifacts/docs_preview/W2/` altında saxlanılan proksi transkriptini sınayın.
- [x] Grafana skrinşotları (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) W2 ön uçuş pəncərəsi üçün çəkilib.
- [x] Göndərilməmişdən əvvəl doldurulmuş rəyçi identifikatorları, sorğu biletləri və təsdiqləmə vaxt nişanları ilə dəvət siyahısı cədvəli (W2 izləyici bölməsinə baxın).

Bu planı yeni saxlayın; izləyici ona istinad edir ki, DOCS-SORA yol xəritəsi W2 dəvətnamələri çıxmazdan əvvəl tam olaraq nə qaldığını görə bilsin.