---
id: security-hardening
lang: az
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Security hardening & pen-test checklist
sidebar_label: Security hardening
description: Harden the developer portal before exposing the Try it sandbox outside the lab.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Baxış

Yol xəritəsi elementi **DOCS-1b** OAuth cihaz koduna giriş, güclü məzmun tələb edir
təhlükəsizlik siyasətləri və önizləmə portalından əvvəl təkrarlana bilən nüfuz testləri
qeyri-laboratoriya şəbəkələrində işləyə bilər. Bu əlavə təhlükə modelini izah edir
repoda həyata keçirilən nəzarətlər və gözdən keçirən canlı yoxlama siyahısı
icra etməlidir.

- **Əhatə dairəsi:** Try it proxy, quraşdırılmış Swagger/RapiDoc panelləri və xüsusi
  `docs/portal/src/components/TryItConsole.jsx` tərəfindən göstərilən konsolu sınayın.
- **Əhatə dairəsi xaricində:** Torii özü (Torii hazırlıq rəyləri əhatə edir) və SoraFS
  nəşriyyat (DOCS-3/7 ilə əhatə olunur).

## Təhdid modeli

| Aktiv | Risk | Azaldılması |
| --- | --- | --- |
| Torii daşıyıcı tokenləri | Sənəd sandboxundan kənarda oğurluq və ya təkrar istifadə | Cihaz koduna giriş (`DOCS_OAUTH_*`) qısamüddətli tokenləri istifadə edir, proksi başlıqları redaktə edir və konsol keşlənmiş etimadnamələri avtomatik bitirir. |
| Bunu cəhd edin proxy | Açıq rele kimi sui-istifadə və ya Torii dərəcə limitlərinin yan keçməsi | `scripts/tryit-proxy*.mjs` mənşəyə icazə verilən siyahıları, sürət məhdudiyyətini, sağlamlıq araşdırmalarını və açıq `X-TryIt-Auth` yönləndirməsini tətbiq edir; heç bir etimadnamə saxlanılmır. |
| Portalın işləmə vaxtı | Saytlararası skript və ya zərərli yerləşdirmə | `docusaurus.config.js` Məzmun-Təhlükəsizlik-Siyasəti, Etibarlı Növlər və İcazələr-Siyasət başlıqlarını daxil edir; daxili skriptlər Docusaurus işləmə vaxtı ilə məhdudlaşır. |
| Müşahidə olunma məlumatları | Çatışmayan telemetriya və ya saxtalaşdırma | `docs/portal/docs/devportal/observability.md` zondları/iş panellərini sənədləşdirir; `scripts/portal-probe.mjs` nəşrdən əvvəl CI-də işləyir. |

Rəqiblərə ictimai baxışa baxan maraqlı istifadəçilər, zərərli aktyorlar daxildir
oğurlanmış bağlantıların sınaqdan keçirilməsi və saxlananları qırmağa çalışan təhlükəsi olan brauzerlər
etimadnamələr. Bütün nəzarətlər etibarlı olmadan əmtəə brauzerlərində işləməlidir
şəbəkələr.

## Tələb olunan nəzarətlər

1. **OAuth cihaz koduna giriş**
   - `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL` konfiqurasiya edin,
     `DOCS_OAUTH_CLIENT_ID` və quraşdırma mühitində əlaqəli düymələr.
   - Sınaq kartı giriş vidjetini (`OAuthDeviceLogin.jsx`) təqdim edir ki,
     cihaz kodunu alır, işarənin son nöqtəsini sorğulayır və tokenləri avtomatik təmizləyir
     müddəti bitdikdən sonra. Əllə Daşıyıcının ləğvi fövqəladə hallar üçün əlçatan qalır
     geri dönüş.
   - OAuth konfiqurasiyası olmadıqda və ya
     ehtiyat TTL-lərin DOCS-1b tərəfindən təyin edilmiş 300-900-cü illər pəncərəsindən kənara sürüşməsi;
     `DOCS_OAUTH_ALLOW_INSECURE=1`-i yalnız birdəfəlik yerli önizləmələr üçün təyin edin.
2. **Proksi qoruyucuları**
   - `scripts/tryit-proxy.mjs` icazə verilən mənşəyi, tarif limitlərini, sorğunu tətbiq edir
     trafiki işarələyərkən ölçü hədləri və yuxarı axın fasilələri
     `X-TryIt-Client` və qeydlərdən redaktor nişanları.
   - `scripts/tryit-proxy-probe.mjs` plus `docs/portal/docs/devportal/observability.md`
     canlılıq zondu və tablosunun qaydalarını müəyyənləşdirin; hərdən əvvəl onları idarə edin
     yayma.
3. **CSP, Etibarlı Növlər, İcazələr-Siyasət**
   - `docusaurus.config.js` indi deterministik təhlükəsizlik başlıqlarını ixrac edir:
     `Content-Security-Policy` (defolt-src özü, ciddi əlaqə/img/skript
     siyahıları, Etibarlı Növlər tələbləri), `Permissions-Policy` və
     `Referrer-Policy: no-referrer`.
   - CSP əlaqə siyahısı OAuth cihaz kodunu və mö'cüzə son nöqtələrini ağ siyahıya alır
     (Yalnız `DOCS_SECURITY_ALLOW_INSECURE=1` istisna olmaqla HTTPS) cihaz girişi işləyir
     qum qutusunu digər mənşələr üçün rahatlaşdırmadan.
   - Başlıqlar birbaşa yaradılan HTML-yə daxil edilir, beləliklə statik hostlar edir
     əlavə konfiqurasiyaya ehtiyac yoxdur. Daxil edilmiş skriptləri məhdudlaşdırın
     Docusaurus yükləmə kəməri.
4. **Runbooks, müşahidə oluna bilənlik və geri çəkilmə**
   - `docs/portal/docs/devportal/observability.md` zondları təsvir edir və
     giriş xətalarını, proxy cavab kodlarını və sorğunu izləyən tablolar
     büdcələr.
   - `docs/portal/docs/devportal/incident-runbooks.md` eskalasiyanı əhatə edir
     qum qutusu sui-istifadə edildikdə yol; ilə birləşdirin
     Son nöqtələri təhlükəsiz çevirmək üçün `scripts/tryit-proxy-rollback.mjs`.

## Qələm testi və buraxılış yoxlama siyahısı

Hər bir önizləmə təşviqi üçün bu siyahını tamamlayın (nəticələri buraxılışa əlavə edin
bilet):

1. **OAuth naqillərini yoxlayın**
   - `DOCS_OAUTH_*` ixracı ilə yerli olaraq `npm run start` işləyin.
   - Təmiz brauzer profilindən Sınaq konsolunu açın və təsdiqləyin
     cihaz kodu axını bir işarə vurur, ömrünü sayar və təmizləyir
     müddəti bitdikdən və ya çıxdıqdan sonra sahə.
2. **Proksi yoxlayın**
   - `npm run tryit-proxy` Torii quruluşuna qarşı, sonra icra edin
     Konfiqurasiya edilmiş nümunə yolu ilə `npm run probe:tryit-proxy`.
   - `authSource=override` qeydləri üçün qeydləri yoxlayın və sürət məhdudiyyətini təsdiqləyin
     pəncərəni aşdığınız zaman sayğacları artırır.
3. **CSP/Etibarlı Növləri Təsdiqləyin**
   - `npm run build` və `build/index.html` açın. `<meta
     http-equiv="Content-Security-Policy">` teqi gözlənilən direktivlərə uyğun gəlir
     və önizləməni yükləyərkən DevTools heç bir CSP pozuntusu göstərmir.
   - Yerləşdirilmiş HTML-ni əldə etmək üçün `npm run probe:portal` (və ya curl) istifadə edin; prob
     indi `Content-Security-Policy`, `Permissions-Policy` və ya
     `Referrer-Policy` meta teqləri yoxdur və ya elan edilmiş dəyərlərdən fərqlidir
     `docusaurus.config.js`-də, beləliklə, idarəetmə rəyçiləri çıxışa etibar edə bilərlər
     kodu yerinə eyeballing curl çıxış.
4. **Müşahidə oluna bilənliyi nəzərdən keçirin**
   - Sınaq proksi tablosunun yaşıl olduğunu yoxlayın (dərəcə limitləri, səhv nisbətləri,
     sağlamlıq tədqiqatı göstəriciləri).
   - `docs/portal/docs/devportal/incident-runbooks.md`-də insident təlimini yerinə yetirin
     host dəyişibsə (yeni Netlify/SoraFS yerləşdirmə).
5. **Nəticələri sənədləşdirin**
   - Buraxılış biletinə ekran görüntülərini/logları əlavə edin.
   - Təmir hesabatı şablonunda hər bir tapıntını qeyd edin
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     beləliklə, sahibləri, SLA-ları və sübutları yenidən yoxlamaq daha sonra asan olur.
   - DOCS-1b yol xəritəsi elementinin yoxlanıla bilən qalması üçün bu yoxlama siyahısına qayıdın.

Hər hansı bir addım uğursuz olarsa, təşviqi dayandırın, bloklama məsələsini bildirin və qeyd edin
`status.md`-də təmir planı.