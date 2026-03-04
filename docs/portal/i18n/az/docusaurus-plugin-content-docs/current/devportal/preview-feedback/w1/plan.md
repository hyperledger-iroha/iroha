---
id: preview-feedback-w1-plan
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Maddə | Təfərrüatlar |
| --- | --- |
| Dalğa | W1 — Partnyorlar və Torii inteqratorları |
| Hədəf pəncərəsi | 2025-ci rüb 3 həftə |
| Artefakt etiketi (planlaşdırılmış) | `preview-2025-04-12` |
| İzləyici problemi | `DOCS-SORA-Preview-W1` |

## Məqsədlər

1. Tərəfdaşın ilkin baxış şərtləri üçün təhlükəsiz hüquqi + idarəetmə təsdiqləri.
2. Dəvət paketində istifadə olunan "Try it it" proksi və telemetriya snapşotlarını səhnələşdirin.
3. Yoxlama məbləği ilə təsdiqlənmiş önizləmə artefaktını və araşdırma nəticələrini yeniləyin.
4. Dəvət göndərilməmişdən əvvəl partnyor siyahısı + sorğu şablonlarını yekunlaşdırın.

## Tapşırıq bölgüsü

| ID | Tapşırıq | Sahibi | Vaxtı | Status | Qeydlər |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | İlkin baxış şərtlərinə əlavə üçün hüquqi təsdiq alın | Sənədlər/DevRel aparıcı → Hüquq | 2025-04-05 | ✅ Tamamlandı | Hüquqi bilet `DOCS-SORA-Preview-W1-Legal` imzalanıb 2025-04-05; PDF izləyiciyə əlavə edilmişdir. |
| W1-P2 | Çək. Bunu sınayın proksi hazırlama pəncərəsi (2025‑04‑10) və proksinin sağlamlığını doğrulayın | Sənədlər/DevRel + Əməliyyatlar | 2025-04-06 | ✅ Tamamlandı | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` icra edilib 2025-04-06; CLI transkripti + `.env.tryit-proxy.bak` arxivləşdirildi. |
| W1-P3 | Önizləmə artefaktını (`preview-2025-04-12`) qurun, `scripts/preview_verify.sh` + `npm run probe:portal` işləyin, arxiv deskriptoru/yoxlama məbləğləri | Portal TL | 2025-04-08 | ✅ Tamamlandı | Artefact + `artifacts/docs_preview/W1/preview-2025-04-12/` altında saxlanılan yoxlama jurnalları; izləyiciyə əlavə edilmiş zond çıxışı. |
| W1-P4 | Tərəfdaş qəbulu formalarını nəzərdən keçirin (`DOCS-SORA-Preview-REQ-P01…P08`), kontaktları təsdiqləyin + NDA | İdarəetmə əlaqəsi | 2025-04-07 | ✅ Tamamlandı | Səkkiz sorğunun hamısı təsdiqləndi (son ikisi 2025-04-11 təmizləndi); izləyicidə əlaqələndirilmiş təsdiqlər. |
| W1-P5 | Layihə dəvət nüsxəsi (`docs/examples/docs_preview_invite_template.md` əsasında), hər bir tərəfdaş üçün `<preview_tag>` və `<request_ticket>` seçin | Sənədlər/DevRel aparıcı | 2025-04-08 | ✅ Tamamlandı | Dəvət layihəsi artefakt linkləri ilə birlikdə 2025‑04‑12 15:00UTC tarixində göndərildi. |

## Uçuşdan əvvəl yoxlama siyahısı

> İpucu: 1-5 addımlarını avtomatik yerinə yetirmək üçün `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`-i işə salın (qurma, yoxlama məbləğinin yoxlanılması, portal araşdırması, keçid yoxlayıcısı və proksi yeniləməsini sınayın). Skript izləyici probleminə əlavə edə biləcəyiniz JSON jurnalını qeyd edir.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` ilə) `build/checksums.sha256` və `build/release.json`-ni bərpa etmək üçün.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` və deskriptorun yanında arxiv `build/link-report.json`.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (və ya `--tryit-target` vasitəsilə müvafiq hədəfi təmin edin); yenilənmiş `.env.tryit-proxy` tətbiq edin və `.bak`-i geri qaytarmaq üçün saxlayın.
6. Jurnal yolları ilə W1 izləyici problemini yeniləyin (deskriptor yoxlama məbləği, zond çıxışı, Proksi dəyişikliyini sınayın, Grafana anlıq görüntüləri).

## Sübut yoxlama siyahısı

- [x] `DOCS-SORA-Preview-W1`-ə əlavə edilmiş imzalanmış hüquqi təsdiq (PDF və ya bilet linki).
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` üçün Grafana ekran görüntüləri.
- [x] `preview-2025-04-12` deskriptor + `artifacts/docs_preview/W1/` altında saxlanılan yoxlama jurnalı.
- [x] `invite_sent_at` vaxt ştampları ilə dəvət siyahısı cədvəli (W1 izləyici jurnalına baxın).
- [x] Rəy artefaktları [`preview-feedback/w1/log.md`](./log.md) hər bir partnyor üçün bir sıra ilə əks etdirilir (2025-04-26 siyahı/telemetri/məsələ datası ilə yenilənib).

Tapşırıqlar irəlilədikcə bu planı yeniləyin; izləyici yol xəritəsini saxlamaq üçün ona istinad edir
yoxlanıla bilər.

## Əlaqə iş prosesi

1. Hər bir rəyçi üçün şablonun dublikatını çıxarın
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   metaməlumatı doldurun və tamamlanmış nüsxəni altında saxlayın
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Dəvətləri, telemetriya yoxlama məntəqələrini və canlı jurnalda açıq məsələləri ümumiləşdirin
   [`preview-feedback/w1/log.md`](./log.md) beləliklə idarəetmə rəyçiləri bütün dalğanı təkrarlaya bilsinlər
   anbardan çıxmadan.
3. Bilik yoxlanışı və ya sorğu ixracı gəldikdə, onları jurnalda qeyd olunan artefakt yoluna əlavə edin
   və izləyici problemini çarpaz bağlayın.