---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
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
| `TRYIT_PROXY_BEARER` | Defolt daşıyıcı nişanı Torii | ünvanına yönləndirildi _boş_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Son istifadəçilərə `X-TryIt-Auth` | vasitəsilə öz tokenlərini təmin etməyə icazə verin `0` |
| `TRYIT_PROXY_MAX_BODY` | Maksimum sorğunun bədən ölçüsü (bayt) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Millisaniyələrlə yuxarı axın fasiləsi | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Müştəri IP üçün tarif pəncərəsinə icazə verilən sorğular | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Məhdudiyyət dərəcəsi üçün sürüşmə pəncərəsi (ms) | `60000` |

Proksi həmçinin `GET /healthz`-i ifşa edir, strukturlaşdırılmış JSON səhvlərini qaytarır və
log çıxışından daşıyıcı tokenləri redaktə edir.

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

## Portal vidcetlərini bağlayın

Tərtibatçı portalını qurarkən və ya xidmət edərkən, vidcetlərin olduğu URL-i təyin edin
proxy üçün istifadə etməlidir:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Aşağıdakı komponentlər bu dəyərləri `docusaurus.config.js`-dən oxuyur:

- **Swagger UI** — `/reference/torii-swagger`-də göstərilmişdir; sorğudan istifadə edir
  daşıyıcı tokenləri avtomatik əlavə etmək üçün interceptor.
- **RapiDoc** — `/reference/torii-rapidoc`-də göstərilmişdir; token sahəsini əks etdirir
  və proxy-yə qarşı sınaq sorğularını dəstəkləyir.
- **Try it console** — API icmalı səhifəsində quraşdırılmışdır; xüsusi göndərmək imkanı verir
  sorğular, başlıqlara baxın və cavab orqanlarını yoxlayın.

İstənilən vidcetdə işarənin dəyişdirilməsi yalnız cari brauzer seansına təsir edir; the
proksi heç vaxt davam etmir və ya təqdim edilmiş nişanı qeyd etmir.

## Müşahidə və əməliyyatlar

Hər sorğu metodu, yolu, mənşəyi, yuxarı axını və statusu ilə bir dəfə qeyd olunur
autentifikasiya mənbəyi (`override`, `default` və ya `client`). Tokenlər heç vaxt deyil
saxlanılır - həm daşıyıcı başlıqlar, həm də `X-TryIt-Auth` dəyərləri əvvəl redaktə edilir
logging - beləliklə, narahat olmadan stdout-u mərkəzi kollektora ötürə bilərsiniz
sirləri sızdırır.

### Sağlamlıq zondları və xəbərdarlıqYerləşdirmə zamanı və ya cədvəl üzrə birləşdirilmiş zondu işə salın:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Ətraf mühit düymələri:

- `TRYIT_PROXY_SAMPLE_PATH` — məşq etmək üçün isteğe bağlı Torii marşrutu (`/proxy` olmadan).
- `TRYIT_PROXY_SAMPLE_METHOD` — defolt olaraq `GET`; yazma marşrutları üçün `POST` olaraq təyin edin.
- `TRYIT_PROXY_PROBE_TOKEN` — nümunə zəngi üçün müvəqqəti daşıyıcı nişanı yeridir.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — defolt 5s fasiləsini ləğv edir.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` üçün isteğe bağlı Prometheus mətn faylı təyinatı.
- `TRYIT_PROXY_PROBE_LABELS` — ölçülərə əlavə edilmiş vergüllə ayrılmış `key=value` cütləri (defolt olaraq `job=tryit-proxy` və `instance=<proxy URL>`).

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