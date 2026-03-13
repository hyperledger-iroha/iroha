---
lang: az
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5246118a539e2031dcafb8cf384ac7d20b8abc28b67ee1555e1b1211779fe390
source_last_modified: "2026-01-22T16:26:46.508367+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
---

Portal trafiki Torii-ə ötürən üç interaktiv səthi birləşdirir:

- `/reference/torii-swagger`-də **Swagger UI** imzalanmış OpenAPI spesifikasiyasını təqdim edir və `TRYIT_PROXY_PUBLIC_URL` təyin edildikdə proksi vasitəsilə sorğuları avtomatik olaraq yenidən yazır.
- `/reference/torii-rapidoc`-də **RapiDoc** fayl yükləmələri və `application/x-norito` üçün yaxşı işləyən məzmun tipi seçiciləri ilə eyni sxemi ifşa edir.
- **Try it sandbox** Norito icmal səhifəsində xüsusi REST sorğuları və OAuth-cihaz girişləri üçün yüngül forma təqdim edir.

Hər üç vidcet yerli **Try-It proxy**-yə sorğu göndərir (`docs/portal/scripts/tryit-proxy.mjs`). Proksi `static/openapi/torii.json`-in `static/openapi/manifest.json`-də imzalanmış digestə uyğun olduğunu yoxlayır, sürət məhdudlaşdırıcısını tətbiq edir, jurnallarda `X-TryIt-Auth` başlıqlarını redaktə edir və hər yuxarı zəngi `X-TryIt-Client` ilə işarələyir, beləliklə, I010 operatoru trafiki yoxlayır.

## Proksi işə salın

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET`, həyata keçirmək istədiyiniz Torii əsas URL-dir.
- `TRYIT_PROXY_ALLOWED_ORIGINS` konsolu daxil etməli olan hər bir portal mənşəyini (yerli inkişaf serveri, istehsal host adı, ilkin baxış URL) daxil etməlidir.
- `TRYIT_PROXY_PUBLIC_URL` `docusaurus.config.js` tərəfindən istehlak edilir və `customFields.tryIt` vasitəsilə vidcetlərə yeridilir.
- `TRYIT_PROXY_BEARER` yalnız `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` olduqda yüklənir; əks halda istifadəçilər konsol və ya OAuth cihaz axını vasitəsilə öz nişanlarını təmin etməlidirlər.
- `TRYIT_PROXY_CLIENT_ID` hər sorğuda daşınan `X-TryIt-Client` etiketini təyin edir.
  Brauzerdən `X-TryIt-Client` təqdim etməyə icazə verilir, lakin dəyərlər kəsilir
  və onların nəzarət simvolları varsa, rədd edilir.

Başlanğıcda proksi `verifySpecDigest`-i işə salır və manifest köhnədirsə, bərpa göstərişi ilə çıxır. Ən yeni Torii spesifikasiyasını yükləmək üçün `npm run sync-openapi -- --latest`-i işə salın və ya fövqəladə hallar üçün `TRYIT_PROXY_ALLOW_STALE_SPEC=1`-i keçin.

Ətraf mühit fayllarını əl ilə redaktə etmədən proksi hədəfi yeniləmək və ya geri qaytarmaq üçün köməkçidən istifadə edin:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Vidcetləri bağlayın

Proksi dinlədikdən sonra portala xidmət edin:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` aşağıdakı düymələri ifşa edir:

| Dəyişən | Məqsəd |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL Swagger, RapiDoc və Cry it sandbox-a daxil edilib. İcazəsiz önizləmə zamanı vidcetləri gizlətmək üçün ayarlanmamış buraxın. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Yaddaşda saxlanılan isteğe bağlı standart nişan. Yerli olaraq `DOCS_SECURITY_ALLOW_INSECURE=1` keçmədiyiniz halda `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` və yalnız HTTPS üçün CSP qoruyucusu (DOCS-1b) tələb olunur. |
| `DOCS_OAUTH_*` | OAuth cihaz axınını (`OAuthDeviceLogin` komponenti) aktivləşdirin ki, rəyçilər portaldan çıxmadan qısamüddətli tokenlər yarada bilsinlər. |

OAuth dəyişənləri mövcud olduqda, qum qutusu konfiqurasiya edilmiş Auth serverindən keçən **Cihaz kodu ilə daxil olun** düyməsini təqdim edir (dəqiq forma üçün `config/security-helpers.js`-ə baxın). Cihaz axını vasitəsilə buraxılan tokenlər yalnız brauzer sessiyasında keşlənir.

## Norito-RPC yükləri göndərilir

1. [Norito sürətli başlanğıcda](./quickstart.md) təsvir edilən CLI və ya fraqmentlərlə `.norito` faydalı yük yaradın. Proksi `application/x-norito` gövdələrini dəyişməz yönləndirir, beləliklə siz `curl` ilə göndərəcəyiniz eyni artefaktdan yenidən istifadə edə bilərsiniz.
2. `/reference/torii-rapidoc` (ikili faydalı yüklər üçün üstünlük verilir) və ya `/reference/torii-swagger` açın.
3. Açılan menyudan istədiyiniz Torii şəklini seçin. Snapshotlar imzalanır; panel `static/openapi/manifest.json`-də qeydə alınmış manifest həzmini göstərir.
4. “Sınayın” qovluğunda `application/x-norito` məzmun növünü seçin, **Fayl seçin** üzərinə klikləyin və yükünüzü seçin. Proksi sorğunu `/proxy/v2/pipeline/submit`-ə yenidən yazır və onu `X-TryIt-Client=docs-portal-rapidoc` ilə işarələyir.
5. Norito cavablarını yükləmək üçün `Accept: application/x-norito` seçin. Swagger/RapiDoc eyni qutuda başlıq seçicisini açır və ikili faylı proksi vasitəsilə geri axır.

Yalnız JSON marşrutları üçün daxil edilmiş Sınaq qutusu tez-tez daha sürətli olur: yolu daxil edin (məsələn, `/v2/accounts/i105.../assets`), HTTP metodunu seçin, lazım olduqda JSON gövdəsini yapışdırın və başlıqları, müddəti və faydalı yükləri yoxlamaq üçün **Sorğu göndər** düyməsini basın.

## Problemlərin aradan qaldırılması

| Simptom | Ehtimal olunan səbəb | Təmir |
| --- | --- | --- |
| Brauzer konsolu CORS xətalarını göstərir və ya sandbox proksi URL-nin çatışmadığını xəbərdar edir. | Proksi işləmir və ya mənbə ağ siyahıya salınmayıb. | Proksini işə salın, `TRYIT_PROXY_ALLOWED_ORIGINS`-in portal hostunuzu əhatə etdiyinə əmin olun və `npm run start`-i yenidən işə salın. |
| `npm run tryit-proxy` "həzm uyğunsuzluğu" ilə çıxış edir. | Torii OpenAPI paketi yuxarıya doğru dəyişdi. | `npm run sync-openapi -- --latest` (və ya `--version=<tag>`) işə salın və yenidən cəhd edin. |
| Vidjetlər `401` və ya `403` qaytarır. | Token çatışmır, vaxtı keçmiş və ya kifayət qədər əhatə dairəsi yoxdur. | OAuth cihaz axınından istifadə edin və ya etibarlı daşıyıcı nişanını sandboxa yapışdırın. Statik tokenlər üçün siz `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ixrac etməlisiniz. |
| Proksidən `429 Too Many Requests`. | IP başına tarif limiti keçildi. | Etibarlı mühitlər və ya tənzimləmə test skriptləri üçün `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS`-i qaldırın. Bütün tarif limitinin rədd edilməsi artımı `tryit_proxy_rate_limited_total`. |

## Müşahidə qabiliyyəti

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs` ətrafında sarğı) `/healthz`-ə zəng edir, istəyə görə nümunə marşrutu həyata keçirir və `probe_success` / I100700 üçün Prometheus mətn faylları yayır. node_exporter ilə inteqrasiya etmək üçün `TRYIT_PROXY_PROBE_METRICS_FILE` konfiqurasiya edin.
- Sayğacları (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) və gecikmə histoqramlarını ifşa etmək üçün `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` seçin. `dashboards/grafana/docs_portal.json` lövhəsi DOCS-SORA SLO-ları tətbiq etmək üçün bu göstəriciləri oxuyur.
- İş vaxtı qeydləri stdout-da canlıdır. Hər bir giriş sorğu id-si, yuxarı axın statusu, autentifikasiya mənbəyi (`default`, `override` və ya `client`) və müddət daxildir; sirr emissiyadan əvvəl redaktə edilir.

`application/x-norito` faydalı yüklərinin dəyişməz olaraq Torii-ə çatdığını təsdiqləməlisinizsə, Jest paketini (`npm test -- tryit-proxy`) işə salın və ya `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` altındakı qurğuları yoxlayın. Reqressiya testləri sıxılmış Norito ikili faylları, imzalanmış OpenAPI manifestləri və proksi aşağı salınma yollarını əhatə edir ki, NRPC buraxılışları daimi sübut izi saxlasın.