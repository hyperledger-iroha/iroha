---
lang: az
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b920e21b96436755f7d37f7b5577465cb3e30016d36340c50f7c6f3a9a46919
source_last_modified: "2025-12-29T18:16:35.116499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sınaq qutusu

Tərtibatçı portalı isteğe bağlı “Sınayın” konsolunu göndərir ki, siz Torii nömrəsinə zəng edə biləsiniz.
sənədləri tərk etmədən son nöqtələr. Konsol sorğuları ötürür
birləşdirilmiş proxy vasitəsilə brauzerlər hələ də CORS məhdudiyyətlərini keçə bilsinlər
dərəcə limitlərinin tətbiqi və autentifikasiya.

## İlkin şərtlər

- Node.js 18.18 və ya daha yeni (portal qurma tələblərinə uyğundur)
- Torii quruluş mühitinə şəbəkə girişi
- Məşq etməyi planlaşdırdığınız Torii marşrutlarına zəng edə bilən daşıyıcı token

Bütün proxy konfiqurasiyası mühit dəyişənləri vasitəsilə həyata keçirilir. Aşağıdakı cədvəl
ən vacib düymələri sadalayır:

| Dəyişən | Məqsəd | Defolt |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Proksinin sorğuları | ünvanına yönləndirdiyi əsas Torii URL **Tələb olunur** |
| `TRYIT_PROXY_LISTEN` | Yerli inkişaf üçün ünvana qulaq asın (format `host:port` və ya `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Proksiyə zəng edə biləcək mənşələrin vergüllə ayrılmış siyahısı | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Hər bir yuxarı sorğu üçün `X-TryIt-Client`-ə yerləşdirilən identifikator | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Defolt daşıyıcı nişanı Torii | ünvanına yönləndirildi _boş_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Son istifadəçilərə `X-TryIt-Auth` | vasitəsilə öz tokenlərini təmin etməyə icazə verin `0` |
| `TRYIT_PROXY_MAX_BODY` | Maksimum sorğunun bədən ölçüsü (bayt) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Millisaniyələrlə yuxarı axın fasiləsi | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Müştəri IP üçün tarif pəncərəsinə icazə verilən sorğular | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Məhdudiyyət dərəcəsi üçün sürüşmə pəncərəsi (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus üslublu metriklərin son nöqtəsi (`host:port` və ya `[ipv6]:port`) üçün əlavə dinləmə ünvanı | _boş (əlil)_ |
| `TRYIT_PROXY_METRICS_PATH` | Metriklərin son nöqtəsi tərəfindən xidmət edilən HTTP yolu | `/metrics` |

Proksi həmçinin `GET /healthz`-i ifşa edir, strukturlaşdırılmış JSON səhvlərini qaytarır və
log çıxışından daşıyıcı tokenləri redaktə edir.

Proksini sənədlər istifadəçilərinə təqdim edərkən `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`-i aktiv edin ki, Swagger və
RapiDoc panelləri istifadəçi tərəfindən təmin edilmiş daşıyıcı tokenləri ötürə bilər. Proksi hələ də tarif limitlərini tətbiq edir,
etimadnamələri redaktə edir və sorğunun defolt işarədən və ya hər bir sorğudan istifadə edib-etmədiyini qeyd edir.
`TRYIT_PROXY_CLIENT_ID`-i `X-TryIt-Client` olaraq göndərmək istədiyiniz etiketə təyin edin
(defolt olaraq `docs-portal`). Proksi zəng edən tərəfindən təmin edilənləri kəsir və təsdiqləyir
`X-TryIt-Client` dəyərləri, bu defolt səviyyəyə qayıdır, beləliklə, mərhələli şlüzlər
brauzer metadatasını əlaqələndirmədən mənşəyi yoxlayın.

## Proksini yerli olaraq başladın

Portalı ilk dəfə qurarkən asılılıqları quraşdırın:

```bash
cd docs/portal
npm install
```

Proksini işə salın və onu Torii nümunəsinə yönəldin:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Skript bağlı ünvanı qeyd edir və sorğuları `/proxy/*` ünvanına yönləndirir.
konfiqurasiya edilmiş Torii mənşəyi.

Soketi bağlamadan əvvəl skript bunu təsdiqləyir
`static/openapi/torii.json`-də qeydə alınan həzm ilə uyğun gəlir
`static/openapi/manifest.json`. Fayllar sürüklənirsə, əmr işarəsi ilə çıxır
səhv edir və `npm run sync-openapi -- --latest`-i işə salmağı əmr edir. İxrac
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` yalnız fövqəladə hallar üçün; vəkil edəcək
bir xəbərdarlıq daxil edin və təmir pəncərələri zamanı bərpa edə bilməyiniz üçün davam edin.

## Portal vidcetlərini bağlayın

Tərtibatçı portalını qurarkən və ya xidmət edərkən, vidcetlərin olduğu URL-i təyin edin
proxy üçün istifadə etməlidir:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Aşağıdakı komponentlər bu dəyərləri `docusaurus.config.js`-dən oxuyur:

- **Swagger UI** — `/reference/torii-swagger`-də göstərilmişdir; qabaqcadan icazə verir
- **MCP reference** - `/reference/torii-mcp`; use this for JSON-RPC `/v1/mcp` agent workflows.
  token mövcud olduqda daşıyıcı sxemi, `X-TryIt-Client` ilə etiketlər sorğuları,
  `X-TryIt-Auth` inyeksiya edir və zaman proxy vasitəsilə zəngləri yenidən yazır.
  `TRYIT_PROXY_PUBLIC_URL` təyin edilib.
- **RapiDoc** — `/reference/torii-rapidoc`-də göstərilmişdir; nişan sahəsini əks etdirir,
  Swagger paneli ilə eyni başlıqları təkrar istifadə edir və proxy-ni hədəfləyir
  URL konfiqurasiya edildikdə avtomatik olaraq.
- **Try it console** — API icmalı səhifəsində quraşdırılmışdır; xüsusi göndərmək imkanı verir
  sorğular, başlıqlara baxın və cavab orqanlarını yoxlayın.

Hər iki paneldə oxuyan **şəkil seçicisi** var
`docs/portal/static/openapi/versions.json`. Həmin indeksi ilə doldurun
`npm run sync-openapi -- --version=<label> --mirror=current --latest` belə
rəyçilər tarixi xüsusiyyətlər arasında atlaya bilər, qeydə alınmış SHA-256 həzminə baxa bilər,
və istifadə etməzdən əvvəl buraxılış şəklinin imzalanmış manifest daşıyıb-daşımadığını təsdiqləyin
interaktiv vidjetlər.

İstənilən vidcetdə işarənin dəyişdirilməsi yalnız cari brauzer seansına təsir edir; the
proksi heç vaxt davam etmir və ya təqdim edilmiş nişanı qeyd etmir.

## Qısamüddətli OAuth tokenləri

Rəyçilər arasında uzun ömürlü Torii tokenlərinin paylanmasının qarşısını almaq üçün Sınayın.
OAuth serverinizə konsol. Aşağıdakı mühit dəyişənləri mövcud olduqda
portal cihaz koduna giriş vidcetini təqdim edir, qısa müddətli daşıyıcı nişanlarını vurur,
və onları avtomatik olaraq konsol formasına daxil edir.

| Dəyişən | Məqsəd | Defolt |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Cihaz Avtorizasiyasının son nöqtəsi (`/oauth/device/code`) | _boş (əlil)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` | qəbul edən token son nöqtəsi _boş_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth müştəri identifikatoru sənədlərin önizləməsi üçün qeydiyyatdan keçdi | _boş_ |
| `DOCS_OAUTH_SCOPE` | Giriş zamanı tələb olunan məkanla ayrılmış əhatə dairələri | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Tokeni | ilə bağlamaq üçün əlavə API auditoriyası _boş_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Təsdiq gözləyərkən minimum sorğu intervalı (ms) | `5000` (<5000ms-dən aşağı qiymətlər rədd edilir) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Geri cihaz kodunun bitmə pəncərəsi (saniyələr) | `600` (300 və 900 arasında qalmalıdır) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Geriyə giriş-token ömrü (saniyələr) | `900` (300 ilə 900 arasında qalmalıdır) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth tətbiqini qəsdən atlayan yerli önizləmələr üçün `1` təyin edin | _ayarlanmamış_ |

Misal konfiqurasiya:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

`npm run start` və ya `npm run build` işlətdiyiniz zaman portal bu dəyərləri daxil edir
`docusaurus.config.js`-də. Yerli önizləmə zamanı Sınaq kartı a göstərir
"Cihaz kodu ilə daxil olun" düyməsi. İstifadəçilər OAuth-da göstərilən kodu daxil edirlər
doğrulama səhifəsi; cihaz axını widgetı müvəffəq etdikdən sonra:

- buraxılmış daşıyıcı nişanını Sınaq konsolu sahəsinə yeridir,
- tələbləri mövcud `X-TryIt-Client` və `X-TryIt-Auth` başlıqları ilə işarələyir,
- qalan ömrü göstərir və
- müddəti bitdikdə nişanı avtomatik təmizləyir.

Əllə Daşıyıcı girişi əlçatan olaraq qalır - istədiyiniz zaman OAuth dəyişənlərini buraxın
rəyçiləri müvəqqəti nişanı özləri yapışdırmağa və ya ixrac etməyə məcbur etmək istəyirlər
`DOCS_OAUTH_ALLOW_INSECURE=1`, anonim girişin olduğu təcrid olunmuş yerli önizləmələr üçün
məqbuldur. İndi konfiqurasiya edilmiş OAuth olmadan qurulanlar təmin etmək üçün sürətlə uğursuz olur
DOCS-1b yol xəritəsi qapısı.

📌 [Təhlükəsizliyin sərtləşdirilməsi və qələm testi yoxlama siyahısını] nəzərdən keçirin (./security-hardening.md)
portalı laboratoriyadan kənarda ifşa etməzdən əvvəl; təhlükə modelini sənədləşdirir,
CSP/Güvənli Növlər profili və indi DOCS-1b-ni əhatə edən nüfuz testi addımları.

## Norito-RPC nümunələri

Norito-RPC sorğuları JSON marşrutları ilə eyni proxy və OAuth santexnikasını paylaşır,
onlar sadəcə olaraq `Content-Type: application/x-norito` təyin edib göndərirlər
NRPC spesifikasiyasında təsvir edilmiş əvvəlcədən kodlanmış Norito faydalı yük
(`docs/source/torii/nrpc_spec.md`).
Anbar `fixtures/norito_rpc/` altında kanonik faydalı yükləri göndərir.
müəlliflər, SDK sahibləri və rəyçilər CI-nin istifadə etdiyi dəqiq baytları təkrarlaya bilərlər.

### Sınaq konsolundan Norito faydalı yük göndərin

1. `fixtures/norito_rpc/transfer_asset.norito` kimi qurğu seçin. Bunlar
   fayllar xam Norito zərfləridir; base64-onları kodlamayın.
2. Swagger və ya RapiDoc-da NRPC son nöqtəsini tapın (məsələn
   `POST /v1/pipeline/submit`) seçin və **Məzmun Tipi** seçicisini dəyişdirin.
   `application/x-norito`.
3. Sorğunun əsas redaktorunu **binar** (Swagger-in "Fayl" rejimi və ya
   RapiDoc-un "İkili/Fayl" seçicisi) və `.norito` faylını yükləyin. Widget
   baytları dəyişdirilmədən proxy vasitəsilə ötürür.
4. Sorğunu təqdim edin. Əgər Torii `X-Iroha-Error-Code: schema_mismatch` qaytarırsa,
   ikili yükləri qəbul edən son nöqtəyə zəng etdiyinizi yoxlayın və
   sxem hashının `fixtures/norito_rpc/schema_hashes.json`-də qeydə alındığını təsdiqləyin
   vurduğunuz Torii quruluşuna uyğun gəlir.

Konsol ən son faylı yaddaşda saxlayır ki, siz eyni faylı yenidən göndərəsiniz
müxtəlif avtorizasiya nişanlarını və ya Torii hostlarını istifadə edərkən faydalı yük. Əlavə edilir
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` sizin iş axını istehsal edir
NRPC-4 qəbul planında istinad edilən sübut paketi (log + JSON xülasəsi),
Baxışlar zamanı Sınayın cavabının ekran görüntüsü ilə gözəl birləşir.

### CLI nümunəsi (qıvrılma)

Eyni qurğular portaldan kənarda `curl` vasitəsilə təkrar oxuna bilər, bu faydalıdır
proksi yoxlanarkən və ya şluz cavablarını sazlayarkən:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

Armaturu `transaction_fixtures.manifest.json`-də sadalanan hər hansı giriş üçün dəyişdirin
və ya öz faydalı yükünüzü `cargo xtask norito-rpc-fixtures` ilə kodlayın. Torii
kanareyka rejimindədir, sınaqdan keçirmə proksisində `curl`-i göstərə bilərsiniz
(`https://docs.sora.example/proxy/v1/pipeline/submit`) eyni şəkildə həyata keçirin
portal vidjetlərinin istifadə etdiyi infrastruktur.

## Müşahidə və əməliyyatlarHər sorğu metodu, yolu, mənşəyi, yuxarı axını və statusu ilə bir dəfə qeyd olunur
autentifikasiya mənbəyi (`override`, `default` və ya `client`). Tokenlər heç vaxt deyil
saxlanılır - həm daşıyıcı başlıqlar, həm də `X-TryIt-Auth` dəyərləri əvvəl redaktə edilir
logging - beləliklə, narahat olmadan stdout-u mərkəzi kollektora ötürə bilərsiniz
sirləri sızdırır.

### Sağlamlıq zondları və xəbərdarlıq

Yerləşdirmə zamanı və ya cədvəl üzrə birləşdirilmiş zondu işə salın:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Ətraf mühit düymələri:

- `TRYIT_PROXY_SAMPLE_PATH` — məşq etmək üçün isteğe bağlı Torii marşrutu (`/proxy` olmadan).
- `TRYIT_PROXY_SAMPLE_METHOD` — defolt olaraq `GET`; yazma marşrutları üçün `POST` olaraq təyin edin.
- `TRYIT_PROXY_PROBE_TOKEN` — nümunə zəngi üçün müvəqqəti daşıyıcı nişanı yeridir.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — defolt 5s fasiləsini ləğv edir.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` üçün isteğe bağlı Prometheus mətn faylı təyinatı.
- `TRYIT_PROXY_PROBE_LABELS` — ölçülərə əlavə edilmiş vergüllə ayrılmış `key=value` cütləri (defolt olaraq `job=tryit-proxy` və `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` — `TRYIT_PROXY_METRICS_LISTEN` aktiv edildikdə uğurla cavab verməli olan isteğe bağlı son nöqtə URL (məsələn, `http://localhost:9798/metrics`).

Nəticələri mətn faylı kollektoruna yazın
yol (məsələn, `/var/lib/node_exporter/textfile_collector/tryit.prom`) və
hər hansı xüsusi etiketlərin əlavə edilməsi:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

Skript metrik faylı atomik şəkildə yenidən yazır ki, kollektorunuz həmişə a oxusun
tam yük.

`TRYIT_PROXY_METRICS_LISTEN` konfiqurasiya edildikdə, təyin edin
`TRYIT_PROXY_PROBE_METRICS_URL` metrikanın son nöqtəsinə köçürün ki, zond sürətlə uğursuz olsun
sıyrılma səthi yox olarsa (məsələn, yanlış konfiqurasiya edilmiş giriş və ya itkin
firewall qaydaları). Tipik istehsal şəraiti
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Yüngül xəbərdarlıq üçün zondu monitorinq yığınınıza tel bağlayın. A Prometheus
məsələn, iki ardıcıl uğursuzluqdan sonra səhifələr:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Metriklərin son nöqtəsi və idarə panelləri

Əvvəl `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (və ya hər hansı host/port cütünü) təyin edin
Prometheus formatlı metriklərin son nöqtəsini ifşa etmək üçün proksi işə salın. Yol
defolt olaraq `/metrics`, lakin vasitəsilə ləğv edilə bilər
`TRYIT_PROXY_METRICS_PATH=/custom`. Hər bir kazıma hər metod üçün sayğacları qaytarır
sorğu cəmləri, tarif limitinin rədd edilməsi, yuxarı xətlər/taymoutlar, proxy nəticələri,
və gecikmə xülasəsi:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP kollektorlarınızı metriklərin son nöqtəsinə yönəldin və yenidən istifadə edin.
mövcud `dashboards/grafana/docs_portal.json` panelləri SRE quyruğu müşahidə edə bilsin
logları təhlil etmədən gecikmələr və rədd etmə sıçrayışları. Proksi avtomatik olaraq
operatorlara yenidən başlamaları aşkar etməyə kömək etmək üçün `tryit_proxy_start_timestamp_ms` nəşr edir.

### Geri qaytarma avtomatlaşdırılması

Hədəf Torii URL-ni yeniləmək və ya bərpa etmək üçün idarəetmə köməkçisindən istifadə edin. Ssenari
əvvəlki konfiqurasiyanı `.env.tryit-proxy.bak`-də saxlayır, beləliklə geri qaytarmalar
tək əmr.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Yerləşdirməniz varsa, `--env` və ya `TRYIT_PROXY_ENV` ilə env fayl yolunu ləğv edin
konfiqurasiyanı başqa yerdə saxlayır.
