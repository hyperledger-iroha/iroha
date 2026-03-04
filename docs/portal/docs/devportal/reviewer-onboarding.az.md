---
lang: az
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f42888a06cb49f9fe53f424ef77c84e2fa3a305f558e202be0fbbd4b3b0ea1d7
source_last_modified: "2025-12-29T18:16:35.114535+00:00"
translation_last_reviewed: 2026-02-07
id: reviewer-onboarding
title: Preview reviewer onboarding
sidebar_label: Reviewer onboarding
description: Process and checklists for enrolling reviewers in the docs portal public preview.
translator: machine-google-reviewed
---

## Baxış

DOCS-SORA inkişaf etdirici portalının mərhələli buraxılışını izləyir. Checksum-qapılı quruluşlar
(`npm run serve`) və bərkidilmiş Cəhd edin, növbəti mərhələni blokdan çıxarın:
ictimai önizləmə geniş şəkildə açılmazdan əvvəl yoxlanılmış rəyçiləri işə götürmək. Bu bələdçi
sorğuları necə toplamaq, uyğunluğu yoxlamaq, giriş təmin etmək və
iştirakçıları təhlükəsiz şəkildə bortdan çıxarın. -a istinad edin
[dəvət axınına ön baxış](./preview-invite-flow.md) kohort planlaması üçün dəvət edin
kadans və telemetriya ixracı; aşağıdakı addımlar görüləcək tədbirlərə diqqət yetirir
rəyçi seçildikdən sonra.

- **Əhatə dairəsi:** sənədlərin önizləməsinə giriş tələb edən rəyçilər (`docs-preview.sora`,
  GitHub Səhifələri qurur və ya SoraFS paketləri) GA-dan əvvəl.
- **Əhatə dairəsi xaricində:** Torii və ya SoraFS operatorları (özlərinin işə qəbulu ilə əhatə olunur)
  dəstlər) və istehsal portalının yerləşdirilməsi (bax
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Rollar və ilkin şərtlər

| Rol | Tipik məqsədlər | Tələb olunan artefaktlar | Qeydlər |
| --- | --- | --- | --- |
| Əsas baxıcı | Yeni təlimatları yoxlayın, tüstü testləri keçirin. | GitHub sapı, Matrix kontaktı, faylda imzalanmış CLA. | Adətən artıq `docs-preview` GitHub komandasında; hələ də sorğu göndərin ki, giriş yoxlanıla bilsin. |
| Tərəfdaş rəyçisi | İctimai yayımdan əvvəl SDK parçalarını və ya idarəetmə məzmununu doğrulayın. | Korporativ e-poçt, qanuni POC, imzalanmış ilkin baxış şərtləri. | Telemetriya + məlumatların işlənməsi tələblərini qəbul etməlidir. |
| İcma könüllüsü | Bələdçilər haqqında istifadəyə dair rəy verin. | GitHub sapı, üstünlük verilən əlaqə, saat qurşağı, CoC qəbulu. | Kohortları kiçik saxlayın; ianəçi müqaviləsini imzalamış rəyçilərə üstünlük verin. |

Bütün rəyçi növləri:

1. Önizləmə artefaktları üçün məqbul istifadə siyasətini qəbul edin.
2. Təhlükəsizlik/müşahidə edilə bilən əlavələri oxuyun
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Hər hansı bir xidmət göstərməzdən əvvəl `docs/portal/scripts/preview_verify.sh`-i işə salmağa razılaşın
   yerli olaraq snapshot.

## Qəbul iş axını

1. Sorğuçudan doldurmağı xahiş edin
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   forma (və ya onu bir məsələyə kopyalayın/yapışdırın). Ən azı ələ keçirin: şəxsiyyət, əlaqə
   metodu, GitHub sapı, nəzərdə tutulan baxış tarixləri və təsdiqi
   təhlükəsizlik sənədləri oxundu.
2. Sorğunu `docs-preview` izləyicisində qeyd edin (GitHub məsələsi və ya idarəetmə
   bilet) və təsdiqləyici təyin edin.
3. İlkin şərtləri təsdiq edin:
   - Fayl üzrə CLA / ianəçi müqaviləsi (və ya tərəfdaş müqaviləsi arayışı).
   - Məqbul-istifadə təsdiqi sorğuda saxlanılır.
   - Riskin qiymətləndirilməsi tamamlandı (məsələn, Hüquqşünaslar tərəfindən təsdiqlənmiş tərəfdaş rəyçiləri).
4. Təsdiq edən sorğuda çıxış edir və izləmə məsələsini istənilən ilə əlaqələndirir
   dəyişiklik idarəetmə girişi (məsələn: `DOCS-SORA-Preview-####`).

## Təminat və alətlər

1. **Artefaktları paylaşın** — Ən son önizləmə təsviri + arxivi təmin edin
   CI iş axını və ya SoraFS pin (`docs-portal-preview` artefakt). Xatırlatmaq
   işlətmək üçün rəyçilər:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Yoxlama məbləğinin tətbiqi ilə xidmət göstərin** — Yoxlama məbləği qapalı yerində nəzərdən keçirənləri qeyd edin
   əmr:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Bu, `scripts/serve-verified-preview.mjs`-dən təkrar istifadə edir ki, heç bir təsdiqlənməmiş quruluş ola bilməz
   təsadüfən işə salındı.

3. **GitHub girişi verin (istəyə görə)** — Rəyçilər dərc olunmamış filiallara ehtiyac duyarsa,
   nəzərdən keçirmə müddətində onları `docs-preview` GitHub komandasına əlavə edin və
   üzvlük dəyişikliyini sorğuda qeyd edin.

4. **Dəstək kanalları ilə əlaqə saxlayın** — Zəng zamanı əlaqəni paylaşın (Matrix/Slack)
   və hadisə proseduru [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetriya + rəy** — Rəyçilərə xatırladın ki, anonim analitika
  toplanmışdır (bax [`observability`](./observability.md)). Rəy bildirin
  dəvətdə istinad edilən forma və ya buraxılış şablonunu daxil edin və tədbiri ilə daxil edin
  [`preview-feedback-log`](./preview-feedback-log) köməkçi belə dalğa xülasəsi
  aktual qalır.

## Rəyçi yoxlama siyahısı

Önizləmədən əvvəl rəyçilər aşağıdakıları yerinə yetirməlidirlər:

1. Yüklənmiş artefaktları yoxlayın (`preview_verify.sh`).
2. `npm run serve` (və ya `serve:verified`) vasitəsilə portalı işə salın.
   checksum qoruyucu aktivdir.
3. Yuxarıda əlaqələndirilmiş təhlükəsizlik və müşahidə olunma qeydlərini oxuyun.
4. OAuth-u sınayın/Cihaz kodu girişindən istifadə edərək konsolu sınayın (əgər varsa) və
   istehsal tokenlərini təkrar istifadə etməyin.
5. Razılaşdırılmış izləyicidə (məsələ, paylaşılan sənəd və ya forma) tapıntıları fayl və etiketləyin
   onları önizləmə buraxılış etiketi ilə.

## Baxıcının öhdəlikləri və işdən kənarlaşdırılma

| Faza | Fəaliyyətlər |
| --- | --- |
| Başlanğıc | Sorğuya daxil olma yoxlama siyahısının əlavə olunduğunu təsdiqləyin, artefaktları + təlimatları paylaşın, [`preview-feedback-log`](./preview-feedback-log) vasitəsilə `invite-sent` girişini əlavə edin və baxış bir həftədən çox davam edərsə, orta nöqtə sinxronizasiyasını planlaşdırın. |
| Monitorinq | Önizləmə telemetriyasını izləyin (qeyri-adi trafiki sınayın, nasazlıqları yoxlayın) və şübhəli hər hansı bir hadisə baş verərsə, insident cədvəlini izləyin. Tapıntılar gəldikdə `feedback-submitted`/`issue-opened` hadisələrini qeyd edin ki, dalğa ölçüləri dəqiq qalsın. |
| Offboarding | Müvəqqəti GitHub və ya SoraFS girişini ləğv edin, `access-revoked`-i qeyd edin, sorğunu arxivləşdirin (rəy xülasəsi + gözlənilməz tədbirlər daxildir) və rəyçi reyestrini yeniləyin. Nəzərdən keçirəndən yerli quruluşları təmizləməsini və [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)-dan yaradılan həzmi əlavə etməyi xahiş edin. |

Rəyçiləri dalğalar arasında çevirərkən eyni prosesi istifadə edin. saxlamaq
repodakı kağız izi (məsələ + şablonlar) DOCS-SORA-in yoxlana bilən qalmasına kömək edir və
idarəetməyə ilkin baxışa girişin sənədləşdirilmiş nəzarətləri izlədiyini təsdiq etməyə imkan verir.

## Dəvət şablonları və izləmə

- Hər bir təbliğatla başlayın
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  fayl. Minimum hüquqi dili ələ keçirir, yoxlama məbləği təlimatlarını nəzərdən keçirir,
  və rəyçilərin məqbul istifadə siyasətini qəbul etməsi gözləntiləri.
- Şablonu redaktə edərkən, `<preview_tag>` üçün yer tutucuları dəyişdirin,
  `<request_ticket>` və əlaqə kanalları. Son mesajın surətini burada saxlayın
  Qəbul bileti beləliklə nəzərdən keçirənlər, təsdiq edənlər və auditorlar müraciət edə bilsinlər
  göndərilən dəqiq ifadə.
- Dəvət göndərildikdən sonra izləmə cədvəlini və ya problemi yeniləyin
  `invite_sent_at` vaxt damğası və gözlənilən bitmə tarixi belədir
  [dəvət axınına ön baxış](./preview-invite-flow.md) hesabat kohortu götürə bilər
  avtomatik.