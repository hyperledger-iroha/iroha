---
lang: az
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

Tərtibatçı portalı Torii REST API üçün “Sınayın” konsolunu göndərir. Bu bələdçi
dəstəkləyən proksi-ni işə salmağı və konsolu səhnələşdirməyə necə qoşmağı izah edir
etimadnamələri ifşa etmədən şlüz.

## İlkin şərtlər

- Iroha repozitoriya yoxlanışı (iş sahəsinin kökü).
- Node.js 18.18+ (portal bazasına uyğundur).
- Torii son nöqtəsi iş stansiyanızdan əldə edilə bilər (tədris və ya yerli).

## 1. OpenAPI snapşotunu yaradın (istəyə görə)

Konsol portal istinad səhifələri ilə eyni OpenAPI faydalı yükünü təkrar istifadə edir. Əgər
siz Torii marşrutlarını dəyişdiniz, snapshotı bərpa edin:

```bash
cargo xtask openapi
```

Tapşırıq `docs/portal/static/openapi/torii.json` yazır.

## 2. Sınaq proksisini işə salın

Repozitor kökündən:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Ətraf dəyişənləri

| Dəyişən | Təsvir |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii əsas URL (tələb olunur). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Proksidən istifadə etməyə icazə verilən mənşələrin vergüllə ayrılmış siyahısı (defolt olaraq `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Könüllü defolt daşıyıcı nişanı bütün etibarlı sorğulara tətbiq edilir. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Zəng edənin `Authorization` başlığını hərfi yönləndirmək üçün `1` olaraq təyin edin. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Yaddaşdaxili sürət məhdudlaşdırıcı parametrləri (defolt: 60 saniyəyə 60 sorğu). |
| `TRYIT_PROXY_MAX_BODY` | Maksimum sorğu yükü qəbul edildi (bayt, defolt 1MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii sorğuları üçün yuxarı axın fasiləsi (defolt 10000ms). |

Proksi ifşa edir:

- `GET /healthz` — hazırlığın yoxlanılması.
- `/proxy/*` — yolu və sorğu sətirini qoruyan etibarlı sorğular.

## 3. Portalı işə salın

Ayrı bir terminalda:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` saytına daxil olun və Sınaq konsolundan istifadə edin. Eyni
mühit dəyişənləri Swagger UI və RapiDoc yerləşdirmələrini konfiqurasiya edir.

## 4. Vahid testlərinin icrası

Proksi sürətli Node əsaslı test paketini nümayiş etdirir:

```bash
npm run test:tryit-proxy
```

Testlər ünvan təhlili, mənşənin idarə edilməsi, tarif məhdudiyyəti və daşıyıcını əhatə edir
inyeksiya.

## 5. Zondların avtomatlaşdırılması və ölçüləri

`/healthz` və nümunə son nöqtəni yoxlamaq üçün birləşdirilmiş zonddan istifadə edin:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Ətraf mühit düymələri:

- `TRYIT_PROXY_SAMPLE_PATH` — məşq etmək üçün isteğe bağlı Torii marşrutu (`/proxy` olmadan).
- `TRYIT_PROXY_SAMPLE_METHOD` — defolt olaraq `GET`; yazma marşrutları üçün `POST` olaraq təyin edin.
- `TRYIT_PROXY_PROBE_TOKEN` — nümunə zəngi üçün müvəqqəti daşıyıcı nişanı yeridir.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — defolt 5s fasiləsini ləğv edir.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` üçün Prometheus mətn faylı təyinatı.
- `TRYIT_PROXY_PROBE_LABELS` — ölçülərə əlavə edilmiş vergüllə ayrılmış `key=value` cütləri (defolt olaraq `job=tryit-proxy` və `instance=<proxy URL>`).

`TRYIT_PROXY_PROBE_METRICS_FILE` təyin edildikdə, skript faylı yenidən yazır
atomik olaraq sizin node_exporter/textfile kollektorunuz həmişə tam görür
faydalı yük. Misal:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Nəticədə ölçüləri Prometheus-ə yönləndirin və nümunə xəbərdarlığını yenidən istifadə edin
`probe_success` `0` səviyyəsinə düşdükdə developer-portal sənədlərini səhifəyə göndərin.

## 6. İstehsalın sərtləşdirilməsinə nəzarət siyahısı

Yerli inkişafdan kənar proksi dərc etməzdən əvvəl:

- TLS-ni proksidən əvvəl dayandırın (əks proxy və ya idarə olunan şlüz).
- Strukturlaşdırılmış girişi konfiqurasiya edin və müşahidə boru kəmərlərinə yönləndirin.
- Daşıyıcı tokenləri çevirin və onları sirr menecerinizdə saxlayın.
- Proksinin `/healthz` son nöqtəsinə və məcmu gecikmə ölçülərinə nəzarət edin.
- Dərəcə məhdudiyyətlərini Torii mərhələ kvotalarınızla uyğunlaşdırın; `Retry-After`-i tənzimləyin
  müştərilərlə əlaqə saxlamaq üçün davranış.