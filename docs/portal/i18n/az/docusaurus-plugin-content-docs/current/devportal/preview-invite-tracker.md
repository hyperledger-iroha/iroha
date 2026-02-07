---
id: preview-invite-tracker
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite tracker
sidebar_label: Preview tracker
description: Wave-by-wave status log for the checksum-gated docs portal preview program.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu izləyici hər bir sənəd portalının önizləmə dalğasını qeyd edir ki, DOCS-SORA sahibləri və
idarəetmə rəyçiləri hansı kohortun aktiv olduğunu, dəvətləri kimin təsdiqlədiyini görə bilər,
və hansı artefaktların hələ də diqqətə ehtiyacı var. Dəvət göndərildikdə onu yeniləyin,
ləğv edilmiş və ya təxirə salınmışdır ki, audit izi anbarda qalsın.

## Dalğa statusu

| Dalğa | Kohort | İzləyici problemi | Təsdiq edən(lər) | Status | Hədəf pəncərəsi | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 – Əsas baxıcılar** | Yoxlama məbləği axınını təsdiqləyən Sənədlər + SDK baxıcıları | `DOCS-SORA-Preview-W0` (GitHub/ops izləyicisi) | Sənədlər/DevRel aparıcısı + Portal TL | 🈴 Tamamlandı | Q2 2025 həftələr 1–2 | Dəvətlər 2025‑03‑25 göndərildi, telemetriya yaşıl qaldı, çıxış xülasəsi 2025‑04‑08 dərc olundu. |
| **W1 – Tərəfdaşlar** | NDA altında SoraFS operatorları, Torii inteqratorları | `DOCS-SORA-Preview-W1` | Sənədlər/DevRel aparıcısı + İdarəetmə əlaqəsi | 🈴 Tamamlandı | 2025-ci rüb 3 həftə | Dəvətlər 2025‑04‑12 → 2025‑04‑26-da səkkiz partnyorun hamısı təsdiqləndi; [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) və [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md)-də çıxış həzmində əldə edilmiş sübutlar. |
| **W2 – İcma** | Seçilmiş icma gözləmə siyahısı (bir dəfə ≤25) | `DOCS-SORA-Preview-W2` | Sənədlər/DevRel aparıcısı + İcma meneceri | 🈴 Tamamlandı | Q3 2025 week 1 (tentative) | Dəvətlər 2025‑06‑15 → 2025‑06‑29-da telemetriya yaşıl rəngdə idi; sübut + [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md)-də əldə edilən tapıntılar. |
| **W3 – Beta kohortları** | Maliyyə/müşahidə edilə bilən beta + SDK tərəfdaşı + ekosistem müdafiəçisi | `DOCS-SORA-Preview-W3` | Sənədlər/DevRel aparıcısı + İdarəetmə əlaqəsi | 🈴 Tamamlandı | 2026-cı rübün 8-ci həftəsi | Dəvətlər 2026‑02‑18 → 2026‑02‑28; həzm + `preview-20260218` dalğası vasitəsilə yaradılan portal məlumatları (bax [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Qeyd: hər bir izləyici məsələsini müvafiq önizləmə sorğusu biletləri ilə əlaqələndirin və
> təsdiqlərin qalması üçün onları `docs-portal-preview` layihəsi çərçivəsində arxivləşdirin
> aşkar edilə bilən.

## Aktiv tapşırıqlar (W0)

- ✅ Uçuşdan əvvəl artefaktlar yeniləndi (GitHub Fəaliyyətləri `docs-portal-preview` 2025‑03‑24 işləyir, deskriptor `preview-2025-03-24` teqindən istifadə edərək `scripts/preview_verify.sh` vasitəsilə təsdiqlənib).
- ✅ Telemetriya əsas xətləri çəkildi (`docs.preview.integrity`, `TryItProxyErrors` tablosunun snapshotı W0 izləyici məsələsində yadda saxlanıldı).
- ✅ Yayım nüsxəsi [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) `preview-2025-03-24` önizləmə etiketi ilə kilidlənib.
- ✅ İlk beş baxıcı üçün daxil olan sorğular (biletlər `DOCS-SORA-Preview-REQ-01` … `-05`).
- ✅ İlk beş dəvət 2025‑03‑25 10:00–10:20 UTC ardıcıl yeddi yaşıl telemetriya günündən sonra göndərildi; `DOCS-SORA-Preview-W0`-də saxlanılan təsdiqlər.
- ✅ Telemetriyaya nəzarət + ev sahibi iş saatları (2025‑03‑31-ə qədər gündəlik qeydiyyat; aşağıda yoxlama məntəqəsi qeydi).
- ✅ Orta nöqtə ilə bağlı rəyləri/məsələləri toplayın və onları etiketləyin `docs-preview/w0` (bax [W0 digest](./preview-feedback/w0/summary.md)).
- ✅ Dalğa xülasəsi + dəvətin çıxış təsdiqlərini dərc edin (çıxış paketi 2025‑04‑08; bax [W0 həzm](./preview-feedback/w0/summary.md)).
- ✅ W3 beta dalğası izlənilir; idarəetmənin nəzərdən keçirilməsindən sonra lazım gəldikdə planlaşdırılan gələcək dalğalar.

## W1 tərəfdaş dalğasının xülasəsi

- ✅ **Hüquqi və idarəetmə təsdiqləri.** Tərəfdaş əlavəsi 2025‑04‑05 imzalanıb; təsdiqlər `DOCS-SORA-Preview-W1`-ə yükləndi.
- ✅ **Telemetriya + Səhnələşdirməni sınayın.** `OPS-TRYIT-147` biletini `docs.preview.integrity`, `docs.preview.integrity`, I18NI0000000000 və I18NI00000000000000 üçün Grafana snapşotları ilə 2025‑04‑06 yerinə yetirilib. arxivləşdirilmişdir.
- ✅ **Artefact + checksum hazırlığı.** `preview-2025-04-12` paketi təsdiqləndi; `artifacts/docs_preview/W1/preview-2025-04-12/` altında saxlanılan deskriptor/yoxlama məbləği/zond qeydləri.
- ✅ **Dəvət siyahısı + göndərmə.** Bütün səkkiz tərəfdaş sorğusu (`DOCS-SORA-Preview-REQ-P01…P08`) təsdiq edildi; dəvətlər 2025‑04‑12 15:00-15:21UTC tarixlərində hər rəyçiyə daxil edilmiş təşəkkürlə göndərildi.
- ✅ **Əlaqə vasitələri.** Gündəlik iş saatları + qeydə alınan telemetriya yoxlama məntəqələri; həzm üçün [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) baxın.
- ✅ **Son siyahı/çıxış jurnalı.** [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) indi 2025‑04‑26-dan etibarən dəvət/qəbul vaxt ştamplarını, telemetriya sübutlarını, viktorina ixracını və artefakt göstəricilərini qeyd edir ki, idarəetmə dalğanı təkrarlaya bilsin.

## Dəvət jurnalı — W0 əsas baxıcıları

| Rəyçi ID | Rol | Bilet tələb | Dəvət göndərildi (UTC) | Gözlənilən çıxış (UTC) | Status | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal baxıcısı | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Aktiv | Qəbul edilmiş yoxlama məbləğinin yoxlanılması; naviqasiya/yan panelin nəzərdən keçirilməsinə diqqət yetirir. |
| sdk-rust-01 | Rust SDK aparıcı | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Aktiv | SDK reseptlərinin sınaqdan keçirilməsi + Norito sürətli başlanğıclar. |
| sdk-js-01 | JS SDK baxıcısı | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Aktiv | Doğrulanır Konsol + ISO axınlarını sınayın. |
| sorafs-ops-01 | SoraFS operator əlaqəsi | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Aktiv | Audit SoraFS runbooks + orkestrasiya sənədləri. |
| müşahidə qabiliyyəti-01 | Müşahidə qabiliyyəti TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Aktiv | Telemetriya/insident əlavələrinin nəzərdən keçirilməsi; Alertmanager əhatə dairəsinə malikdir. |

Bütün dəvətlər eyni `docs-portal-preview` artefaktına istinad edir (2025‑03‑24,
etiketi `preview-2025-03-24`) və ələ keçirilən yoxlama transkripti
`DOCS-SORA-Preview-W0`. İstənilən əlavələr/pauzalar hər iki cədvələ daxil edilməlidir
yuxarıda və növbəti dalğaya keçməzdən əvvəl izləyici problemi.

## Yoxlama məntəqəsi jurnalı — W0

| Tarix (UTC) | Fəaliyyət | Qeydlər |
| --- | --- | --- |
| 2025-03-26 | Telemetriya bazasına baxış + iş saatları | `docs.preview.integrity` + `TryItProxyErrors` yaşıl qaldı; iş saatları bütün rəyçilərin yoxlama məbləğinin yoxlanışını tamamladığını təsdiqlədi. |
| 2025-03-27 | Orta nöqtə ilə bağlı rəy həzmi dərc edildi | Xülasə [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); `docs-preview/w0` etiketləri kimi daxil edilmiş iki kiçik naviqasiya problemi, heç bir insident bildirilməyib. |
| 2025-03-31 | Son həftə telemetriya yerində yoxlama | Çıxışdan əvvəl son iş saatları; Rəyçilər yolda qalan sənəd tapşırıqlarını təsdiqlədilər, heç bir xəbərdarlıq verilmədi. |
| 2025-04-08 | Çıxış xülasəsi + bağlamaları dəvət | Tamamlanmış rəylər, ləğv edilmiş müvəqqəti giriş, [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) daxilində arxivləşdirilmiş tapıntılar; izləyici W1-i hazırlamazdan əvvəl yeniləndi. |

## Dəvət jurnalı — W1 tərəfdaşları

| Rəyçi ID | Rol | Bilet tələb | Dəvət göndərildi (UTC) | Gözlənilən çıxış (UTC) | Status | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (AB) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Tamamlandı | Təslim edilmiş orkestr əməliyyatları ilə bağlı rəy 2025‑04‑20; 15:05UTC-dən çıxın. |
| sorafs-op-02 | SoraFS operatoru (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Tamamlandı | `docs-preview/w1`-də buraxılış təlimatı şərhləri daxil edilmişdir; 15:10UTC-dən çıxın. |
| sorafs-op-03 | SoraFS operator (ABŞ) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Tamamlandı | Mübahisə/qara siyahı redaktələri qaldırıldı; 15:12UTC-dən çıxın. |
| torii-int-01 | Torii inteqratoru | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Tamamlandı | Bunu cəhd edin, auth prospekti qəbul edildi; 15:14UTC-dən çıxın. |
| torii-int-02 | Torii inteqrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Tamamlandı | RPC/OAuth sənəd şərhləri daxil edilmişdir; 15:16UTC-dən çıxın. |
| sdk-partner-01 | SDK tərəfdaşı (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Tamamlandı | Preview bütövlüyü rəyi birləşdirildi; 15:18UTC-dən çıxın. |
| sdk-partner-02 | SDK tərəfdaşı (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Tamamlandı | Telemetriya/redasiya nəzərdən keçirildi; 15:22UTC-dən çıxın. |
| Gateway-ops-01 | Gateway operatoru | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Tamamlandı | Gateway DNS runbook şərhləri təqdim edildi; 15:24UTC-dən çıxın. |

## Yoxlama məntəqəsi jurnalı — W1

| Tarix (UTC) | Fəaliyyət | Qeydlər |
| --- | --- | --- |
| 2025‑04‑12 | Dəvət göndərilməsi + artefaktın yoxlanılması | Bütün səkkiz tərəfdaşa `preview-2025-04-12` deskriptoru/arxivi ilə e-poçt göndərildi; izləyicidə saxlanılan təşəkkürlər. |
| 2025-04-13 | Telemetriyanın əsas icmalı | `docs.preview.integrity`, `TryItProxyErrors` və `DocsPortal/GatewayRefusals` idarə panelləri nəzərdən keçirildi — bütün lövhədə yaşıl; iş saatları təsdiqlənmiş yoxlama məbləğinin yoxlanılması tamamlandı. |
| 2025-04-18 | Orta dalğa iş saatları | `docs.preview.integrity` yaşıl qaldı; `docs-preview/w1` altında daxil olmuş iki doc nits (nav mətni + Sınaq ekran görüntüsü). |
| 2025‑04‑22 | Son telemetriya yerində yoxlama | Proksi + idarə panelləri hələ də sağlamdır; heç bir yeni problem qaldırılmadı, çıxışdan əvvəl izləyicidə qeyd edildi. |
| 2025‑04‑26 | Çıxış xülasəsi + bağlamaları dəvət | Bütün tərəfdaşlar nəzərdən keçirmənin tamamlandığını təsdiqlədi, dəvətlər ləğv edildi, sübut [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) daxilində arxivləşdirildi. |

## W3 beta kohortunun xülasəsi

- ✅ 2026‑02‑18-də yoxlama məbləğinin yoxlanılması ilə göndərilən dəvətlər + həmin gün daxil edilmiş təşəkkürlər.
- ✅ `DOCS-SORA-Preview-20260218` idarəetmə məsələsi ilə `docs-preview/20260218` altında toplanmış rəy; həzm + xülasə `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` vasitəsilə yaradıldı.
- ✅ Son telemetriya yoxlanışından sonra 2026‑02‑28 tarixində giriş ləğv edildi; izləyici + portal cədvəlləri W3-ü tamamlanmış kimi göstərmək üçün yeniləndi.

## Dəvət jurnalı — W2 icması| Rəyçi ID | Rol | Bilet tələb | Dəvət göndərildi (UTC) | Gözlənilən çıxış (UTC) | Status | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| comm-cild-01 | İcma rəyçisi (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Tamamlandı | Akt 16:06UTC; SDK sürətli başlanğıclarına diqqət yetirmək; çıxış 2025-06-29 təsdiqləndi. |
| comm-cild-02 | İcma rəyçisi (İdarəetmə) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Tamamlandı | İdarəetmə/SNS nəzərdən keçirildi; çıxış 2025-06-29 təsdiqləndi. |
| comm-cild-03 | İcma rəyçisi (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Tamamlandı | Norito ətraflı rəy qeyd edildi; çıxın 2025‑06‑29. |
| comm-cild-04 | İcma rəyçisi (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Tamamlandı | SoraFS runbook nəzərdən keçirildi; çıxın 2025‑06‑29. |
| comm-cild-05 | İcma rəyçisi (əlçatanlıq) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Tamamlandı | Əlçatanlıq/UX qeydləri paylaşıldı; çıxın 2025‑06‑29. |
| comm-cild-06 | İcma rəyçisi (Lokallaşdırma) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Tamamlandı | Lokallaşdırma ilə bağlı rəy qeyd edildi; çıxın 2025‑06‑29. |
| comm-cild-07 | İcma rəyçisi (Mobil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Tamamlandı | Mobil SDK sənəd çekləri çatdırıldı; çıxın 2025‑06‑29. |
| comm-cild-08 | İcma rəyçisi (Müşahidə qabiliyyəti) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Tamamlandı | Müşahidə edilə bilən əlavənin nəzərdən keçirilməsi həyata keçirilib; çıxın 2025‑06‑29. |

## Yoxlama məntəqəsi jurnalı — W2

| Tarix (UTC) | Fəaliyyət | Qeydlər |
| --- | --- | --- |
| 2025-06-15 | Dəvət göndərilməsi + artefaktın yoxlanılması | 8 icma rəyçisi ilə paylaşılan `preview-2025-06-15` təsviri/arxivi; izləyicidə saxlanılan təşəkkürlər. |
| 2025-06-16 | Telemetriyanın əsas icmalı | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` idarə panelləri yaşıl; Bunu cəhd edin, proksi qeydləri aktiv icma nişanlarını göstərir. |
| 2025-06-18 | Ofis saatları və problem triajı | Toplanmış iki təklif (`docs-preview/w2 #1` alət ipucu mətni, `#2` lokalizasiya yan paneli) — hər ikisi Sənədə yönləndirilir. |
| 2025-06-21 | Telemetriya yoxlanışı + sənəd düzəlişləri | `docs-preview/w2 #1/#2` ünvanlı sənədlər; tablosuna hələ də yaşıl, heç bir insident. |
| 2025-06-24 | Son həftə iş saatları | Rəyçilər qalan rəy təqdimatlarını təsdiqlədilər; yanğın xəbərdarlığı yoxdur. |
| 2025-06-29 | Çıxış xülasəsi + bağlamaları dəvət | Qeydlər qeydə alınıb, ilkin baxışa giriş ləğv edilib, telemetriya anlıq görüntüləri + arxivləşdirilən artefaktlar (bax [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Ofis saatları və problem triajı | `docs-preview/w1` altında daxil edilmiş iki sənəd təklifi; heç bir insident və ya siqnallar işə salınmadı. |

## Hesabat qarmaqları

- Hər çərşənbə, yuxarıdakı izləyici cədvəlini və aktiv dəvət məsələsini yeniləyin
  qısa status qeydi ilə (göndərilən dəvətlər, aktiv rəyçilər, hadisələr).
- Dalğa bağlandıqda, rəyin xülasə yolunu əlavə edin (məsələn,
  `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) və onu əlaqələndirin
  `status.md`.
- [Dəvət axınına ön baxış] (./preview-invite-flow.md) ilə bağlı hər hansı bir fasilə meyarı olarsa
  tetikleyin, dəvətləri davam etdirməzdən əvvəl bura düzəliş addımlarını əlavə edin.