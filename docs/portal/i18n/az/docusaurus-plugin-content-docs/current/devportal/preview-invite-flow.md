---
id: preview-invite-flow
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite flow
sidebar_label: Preview invite flow
description: Sequencing, evidence, and communications plan for the docs portal public preview waves.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Məqsəd

Yol xəritəsi elementi **DOCS-SORA** rəyçinin işə qəbulunu və ictimai baxışı çağırır
portal betadan çıxmazdan əvvəl proqramı son blokerlər kimi dəvət edin. Bu səhifə
hansı artefaktların əvvəl göndərilməli olduğu hər dəvət dalğasının necə açılacağını təsvir edir
dəvətlər çıxır və axının yoxlanıla biləcəyini necə sübut etmək olar. Yanında istifadə edin:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) üçün
  hər bir rəyçi ilə işləmə.
- Yoxlama məbləği üçün [`devportal/preview-integrity-plan`](./preview-integrity-plan.md)
  zəmanət verir.
- [`devportal/observability`](./observability.md) telemetriya ixracı və
  xəbərdarlıq qarmaqları.

## Dalğa planı

| Dalğa | Tamaşaçılar | Giriş meyarları | Çıxış meyarları | Qeydlər |
| --- | --- | --- | --- | --- |
| **W0 – Əsas baxıcılar** | İlk gün məzmununu təsdiqləyən Sənəd/SDK baxıcıları. | `docs-portal-preview` GitHub komandası yaşayır, `npm run serve` yoxlama qapısı yaşıl, Alertmanager 7 gün ərzində sakitdir. | Bütün P0 sənədləri nəzərdən keçirildi, geridə qalanlar qeyd edildi, heç bir maneə törətmədi. | Axını təsdiqləmək üçün istifadə olunur; dəvət e-poçtu yoxdur, sadəcə önizləmə artefaktlarını paylaşın. |
| **W1 – Tərəfdaşlar** | SoraFS operatorları, Torii inteqratorları, NDA çərçivəsində idarəetmə rəyçiləri. | W0 çıxdı, hüquqi şərtlər təsdiqləndi, Proksi sınaqdan keçirildi. | Toplanmış tərəfdaş qeydiyyatı (məsələ və ya imzalanmış forma), telemetriya ≤10 eyni zamanda nəzərdən keçirənləri göstərir, 14 gün ərzində heç bir təhlükəsizlik reqresiyası yoxdur. | Dəvət şablonunu tətbiq edin + sorğu biletləri. |
| **W2 – İcma** | İcma gözləmə siyahısından seçilmiş töhfəçilər. | W1-dən çıxdı, insident təlimləri məşq edildi, ictimai tez-tez verilən suallar yeniləndi. | Rəy həzm edildi, geriyə qaytarılmadan ilkin baxış boru kəməri ilə göndərilən ≥2 sənəd buraxılışı. | Paralel dəvətləri məhdudlaşdırın (≤25) və həftəlik paket. |

`status.md` daxilində və önizləmə sorğusunda hansı dalğanın aktiv olduğunu sənədləşdirin
izləyici beləliklə idarəetmə proqramı bir baxışda harada oturduğunu görə bilsin.

## Uçuşdan əvvəl yoxlama siyahısı

Dalğa üçün dəvətləri planlaşdırmadan **əvvəl** bu əməliyyatları tamamlayın:

1. **CI artefaktları mövcuddur**
   - Ən son `docs-portal-preview` + deskriptoru yükləyib
     `.github/workflows/docs-portal-preview.yml`.
   - SoraFS pin `docs/portal/docs/devportal/deploy-guide.md`-də qeyd olunub
     (kəsmə təsviri mövcuddur).
2. **Yoxlama məbləğinin icrası**
   - `docs/portal/scripts/serve-verified-preview.mjs` vasitəsilə çağırılır
     `npm run serve`.
   - `scripts/preview_verify.sh` təlimatları macOS + Linux-da sınaqdan keçirilmişdir.
3. **Telemetri bazası**
   - `dashboards/grafana/docs_portal.json` sağlam göstərir Trafik və cəhd edin
     `docs.preview.integrity` siqnalı yaşıldır.
   - Ən son `docs/portal/docs/devportal/observability.md` əlavəsi ilə yeniləndi
     Grafana bağlantıları.
4. **İdarəetmə artefaktları**
   - Dəvət izləyici məsələsi hazırdır (hər dalğa üçün bir məsələ).
   - Rəyçi reyestrinin şablonu kopyalandı (bax
     [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Məsələ ilə bağlı hüquqi və SRE tələb olunan təsdiqlər.

Hər hansı bir məktub göndərməzdən əvvəl dəvət izləyicisində uçuş öncəsi tamamlanmasını qeyd edin.

## Axın addımları

1. **Namizədləri seçin**
   - Gözləmə siyahısı cədvəlindən və ya tərəfdaş növbəsindən çəkin.
   - Hər bir namizədin doldurulmuş sorğu şablonuna malik olduğundan əmin olun.
2. **Girişi təsdiq edin**
   - Dəvət izləyicisi məsələsinə təsdiqləyici təyin edin.
   - İlkin şərtləri yoxlayın (CLA/müqavilə, məqbul istifadə, təhlükəsizlik qısası).
3. **Dəvət göndərin**
   - doldurun
     [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
     yer tutucular (`<preview_tag>`, `<request_ticket>`, kontaktlar).
   - Təsviri + arxiv hashını əlavə edin, URL-ni sınayın və dəstək olun
     kanallar.
   - Son e-poçtu (və ya Matrix/Slack transkriptini) buraxılışda saxlayın.
4. **İzlənməni izləyin**
   - `invite_sent_at`, `expected_exit_at` ilə dəvət izləyicisini yeniləyin və
     statusu (`pending`, `active`, `complete`, `revoked`).
   - Audit qabiliyyətinə dair rəyçinin qəbul sorğusuna keçid.
5. **Monitor telemetriya**
   - `docs.preview.session_active` və `TryItProxyErrors` xəbərdarlıqlarına baxın.
   - Telemetriya əsas xəttdən kənara çıxarsa, hadisəni qeyd edin və qeyd edin
     dəvət girişinin yanında nəticə.
6. **Rəy toplayın və çıxın**
   - Rəy daxil olduqda və ya `expected_exit_at` keçdikdən sonra dəvətləri bağlayın.
   - Dalğa məsələsini qısa xülasə ilə yeniləyin (tapıntılar, hadisələr, sonrakı
     hərəkətlər) növbəti kohorta keçməzdən əvvəl.

## Sübut və hesabat

| Artefakt | Harada saxlamaq olar | Kadansı yeniləyin |
| --- | --- | --- |
| Dəvət izləyicisi problemi | `docs-portal-preview` GitHub layihəsi | Hər dəvətdən sonra yeniləyin. |
| Rəyçi siyahısı ixracı | `docs/portal/docs/devportal/reviewer-onboarding.md` əlaqəli reyestr | Həftəlik. |
| Telemetriya görüntüləri | `docs/source/sdk/android/readiness/dashboards/<date>/` (telemetriya paketindən təkrar istifadə) | Dalğa başına + hadisələrdən sonra. |
| Əlaqə həzmi | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (dalğa başına qovluq yaradın) | Dalğa çıxdıqdan sonra 5 gün ərzində. |
| İdarə Heyətinin iclas qeydi | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Hər DOCS-SORA idarəetmə sinxronizasiyasından əvvəl doldurun. |

`cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`-i işə salın
maşın tərəfindən oxuna bilən hadisə həzmini hazırlamaq üçün hər partiyadan sonra. Göstərilənləri əlavə edin
JSON-u dalğa probleminə göndərin ki, idarəetməni nəzərdən keçirənlər dəvət saymalarını olmadan təsdiq edə bilsinlər
bütün qeydi təkrar oxuyur.

Dalğa bitdikdə sübut siyahısını `status.md`-ə əlavə edin ki, yol xəritəsi
giriş tez bir zamanda yenilənə bilər.

## Geri qaytarma və fasilə meyarları

Aşağıdakılardan hər hansı biri baş verdikdə dəvət axınını dayandırın (və idarəetməni xəbərdar edin):

- Geri qaytarmağı tələb edən bir cəhd edin proksi hadisəsi (`npm run manage:tryit-proxy`).
- Yorğunluq xəbərdarlığı: 7 gün ərzində yalnız önizləmə üçün son nöqtələr üçün >3 xəbərdarlıq səhifəsi.
- Uyğunluq boşluğu: imzalanmış şərtlər olmadan və ya daxil edilmədən göndərilən dəvət
  sorğu şablonu.
- Dürüstlük riski: yoxlama məbləği uyğunsuzluğu `scripts/preview_verify.sh` tərəfindən aşkar edildi.

Yalnız dəvət izləyicisində düzəliş sənədləşdirildikdən sonra davam edin və
telemetriya tablosunun ən azı 48 saat stabil olduğunu təsdiqləmək.