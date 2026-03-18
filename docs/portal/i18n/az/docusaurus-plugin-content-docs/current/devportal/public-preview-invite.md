---
id: public-preview-invite
lang: az
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Public preview invite playbook
sidebar_label: Preview invite playbook
description: Checklist for announcing the docs portal preview to external reviewers.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Proqram məqsədləri

Bu oyun kitabı bir dəfə ictimai önizləməni necə elan etməyi və işə salmağı izah edir
rəyçinin işə qəbul iş prosesi canlıdır. DOCS-SORA yol xəritəsini dürüst saxlayır
hər dəvət gəmisinin yoxlanıla bilən artefaktlarla təmin edilməsi, təhlükəsizlik təlimatı və a
aydın əks əlaqə yolu.

- **Auditoriya:** icma üzvlərinin, tərəfdaşların və baxıcıların seçilmiş siyahısı
  önizləmə məqbul istifadə siyasətini imzaladı.
- **Tavanlar:** standart dalğa ölçüsü ≤ 25 rəyçi, 14 günlük giriş pəncərəsi, insident
  24 saat ərzində cavab.

## Gating yoxlama siyahısını işə salın

Hər hansı dəvət göndərməzdən əvvəl bu tapşırıqları yerinə yetirin:

1. CI-də yüklənmiş ən son önizləmə artefaktları (`docs-portal-preview`,
   yoxlama cəmi manifest, deskriptor, SoraFS paketi).
2. `npm run --prefix docs/portal serve` (yoxlama məbləği qapalı) eyni etiketdə sınaqdan keçirilmişdir.
3. Rəyçinin uçuş biletləri təsdiqləndi və dəvət dalğası ilə əlaqələndirildi.
4. Təhlükəsizlik, müşahidə oluna bilənlik və insident sənədləri təsdiq edilmişdir
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Rəy forması və ya buraxılış şablonu hazırlanmışdır (ciddilik üçün sahələr,
   reproduksiya addımları, ekran görüntüləri və ətraf mühit haqqında məlumat).
6. Docs/DevRel + Governance tərəfindən nəzərdən keçirilən elanın surəti.

## Dəvət paketi

Hər bir dəvət aşağıdakıları əhatə etməlidir:

1. **Təsdiqlənmiş artefaktlar** — SoraFS manifest/plan və ya GitHub artefaktını təmin edin
   bağlantılar üstəgəl yoxlama cəmi manifest və deskriptor. Doğrulamaya istinad edin
   açıq şəkildə əmr edin ki, rəyçilər saytı işə salmazdan əvvəl onu işlətsinlər.
2. **Xidmət təlimatları** — Checksum-qapalı önizləmə əmrini daxil edin:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Təhlükəsizlik xatırlatmaları** — Tokenlərin avtomatik bitdiyini bildirin, linklər
   paylaşılmamalı və hadisələr dərhal bildirilməlidir.
4. **Əlaqə kanalı** — Problem şablonuna/formasına keçid edin və cavabı aydınlaşdırın
   vaxt gözləntiləri.
5. **Proqram tarixləri** — Başlama/bitmə tarixlərini, iş saatlarını və ya sinxron görüşləri təmin edin,
   və növbəti yeniləmə pəncərəsi.

E-poçt nümunəsi
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
bu tələbləri əhatə edir. Yer tutanları yeniləyin (tarixlər, URL-lər, kontaktlar)
göndərməzdən əvvəl.

## Önizləmə hostunu ifşa edin

İlkin baxış aparıcısını yalnız işə qəbul tamamlandıqdan və bilet dəyişdirildikdən sonra təbliğ edin
təsdiq edilir. [Önizləmə hostuna məruz qalma təlimatına](./preview-host-exposure.md) baxın
bu bölmədə istifadə olunan başdan-başa qurma/nəşr etmə/doğrulama addımları üçün.

1. **Yaradın və qablaşdırın:** Buraxılış etiketini möhürləyin və deterministik istehsal edin
   artefaktlar.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   Pin skripti yazır `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   və `portal.dns-cutover.json` `artifacts/sorafs/` altında. Həmin fayllara əlavə edin
   dalğa dəvət edin ki, hər bir rəyçi eyni bitləri yoxlaya bilsin.

2. **Önizləmə ləqəbini dərc edin:** `--skip-submit` olmadan əmri yenidən icra edin
   (təchizat `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` və
   idarəetmə tərəfindən verilmiş ləqəb sübutu). Skript manifestə bağlanacaq
   `docs-preview.sora` və `portal.manifest.submit.summary.json` plus yayır
   Sübut dəsti üçün `portal.pin.report.json`.

3. **Yerləşdirməni yoxlayın:** Təxəllənin həll edildiyini və yoxlama məbləğinin uyğunluğunu təsdiqləyin
   dəvət göndərməzdən əvvəl etiket.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) kimi əlinizdə saxlayın
   geri qaytarma, beləliklə, rəyçilər önizləmə kənarı sönərsə yerli nüsxəni çıxara bilsinlər.

## Rabitə qrafiki

| Gün | Fəaliyyət | Sahibi |
| --- | --- | --- |
| D-3 | Dəvət nüsxəsini yekunlaşdırın, artefaktları yeniləyin, quru işləmə yoxlaması | Sənədlər/DevRel |
| D-2 | İdarəetmə qeydiyyatı + dəyişiklik bileti | Sənədlər/DevRel + İdarəetmə |
| D-1 | Şablondan istifadə edərək dəvət göndərin, alıcı siyahısı ilə izləyicini yeniləyin | Sənədlər/DevRel |
| D | Başlanğıc çağırışı / iş saatları, telemetriya tablosuna nəzarət | Sənədlər/DevRel + Zəng üzrə |
| D+7 | Orta nöqtə geribildirim həzmi, triajın bloklanması məsələləri | Sənədlər/DevRel |
| D+14 | Dalğanı bağlayın, müvəqqəti girişi ləğv edin, xülasəni `status.md`-də dərc edin | Sənədlər/DevRel |

## Giriş izləmə və telemetriya

1. Hər bir alıcını qeyd edin, dəvət vaxtı damğası və ləğv tarixi
   geribildirim qeydçisi (bax
   [`preview-feedback-log`](./preview-feedback-log)) beləliklə hər dalğa paylaşır
   eyni sübut izi:

   ```bash
   # Append a new invite event to artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Dəstəklənən hadisələr `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` və `access-revoked`. Günlük yaşayır
   Defolt olaraq `artifacts/docs_portal_preview/feedback_log.json`; əlavə edin
   razılıq formaları ilə birlikdə dəvət dalğası bileti. Xülasə köməkçisindən istifadə edin
   bağlanma qeydindən əvvəl yoxlanıla bilən bir hesabat hazırlamaq üçün:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   JSON xülasəsi hər dalğa üçün dəvətləri, açıq alıcıları, rəyləri sadalayır
   sayılar və ən son hadisənin vaxt möhürü. Köməkçi tərəfindən dəstəklənir
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   beləliklə, eyni iş axını yerli və ya CI-də işləyə bilər. Həzm şablonundan istifadə edin
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   dalğa xülasəsini dərc edərkən.
2. Telemetriya panellərini dalğa üçün istifadə olunan `DOCS_RELEASE_TAG` ilə etiketləyin.
   sünbüllər dəvət kohortları ilə əlaqələndirilə bilər.
3. Yerləşdirmədən sonra `npm run probe:portal -- --expect-release=<tag>`-i işə salın
   önizləmə mühitinin düzgün buraxılış metadatasını reklam etdiyini təsdiqləyin.
4. Runbook şablonunda hər hansı insidentləri qeyd edin və onları kohorta əlaqələndirin.

## Rəy və bağlanma

1. Paylaşılan sənəd və ya buraxılış lövhəsində rəyi ümumiləşdirin. Elementləri ilə etiketləyin
   `docs-preview/<wave>` beləliklə, yol xəritəsi sahibləri onları asanlıqla sorğulaya bilsinlər.
2. Dalğa hesabatını doldurmaq üçün ilkin baxış qeydinin xülasə çıxışından istifadə edin, sonra
   `status.md`-də kohortu ümumiləşdirin (iştirakçılar, əsas tapıntılar, planlaşdırılan
   düzəlişlər) və DOCS-SORA mərhələ dəyişibsə, `roadmap.md`-i yeniləyin.
3. Buradan kənara çıxma addımlarını izləyin
   [`reviewer-onboarding`](./reviewer-onboarding.md): girişi ləğv edin, arxivləşdirin
   rica edir və iştirakçılara təşəkkür edir.
4. Artefaktları təravətləndirərək, yoxlama qapılarını yenidən işə salmaqla növbəti dalğanı hazırlayın,
   və dəvət şablonunun yeni tarixlərlə yenilənməsi.

Ardıcıl olaraq bu kitabçanın tətbiqi önizləmə proqramını yoxlanıla bilir və saxlayır
Portal GA-ya yaxınlaşdıqca, Docs/DevRel-ə dəvətləri ölçmək üçün təkrarlanan üsul verir.