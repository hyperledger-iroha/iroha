---
lang: mn
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

Хөгжүүлэгчийн портал нь Torii REST API-д зориулсан "Оролдоод үзээрэй" консолыг илгээдэг. Энэхүү гарын авлага
Дэмжих проксиг хэрхэн ажиллуулж, консолыг үе шаттай холбох талаар тайлбарладаг
итгэмжлэлийг ил гаргахгүйгээр гарц.

## Урьдчилсан нөхцөл

- Iroha репозиторыг шалгах (ажлын талбарын үндэс).
- Node.js 18.18+ (порталын суурьтай таарч байна).
- Torii эцсийн цэгийг таны ажлын станцаас авах боломжтой (үе шатлал эсвэл орон нутгийн).

## 1. OpenAPI агшин зуурын зургийг үүсгэх (заавал биш)

Консол нь порталын лавлах хуудсуудтай ижил OpenAPI ачааллыг дахин ашигладаг. Хэрэв
та Torii маршрутуудыг өөрчилсөн тул агшин зуурын зургийг дахин үүсгэнэ үү:

```bash
cargo xtask openapi
```

Даалгавар нь `docs/portal/static/openapi/torii.json` гэж бичдэг.

## 2. Try It прокси-г эхлүүлнэ үү

Хадгалах сангийн үндэсээс:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Хүрээлэн буй орчны хувьсагчид

| Хувьсагч | Тодорхойлолт |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii үндсэн URL (шаардлагатай). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Прокси ашиглахыг зөвшөөрсөн гарал үүслийн жагсаалтыг таслалаар тусгаарласан (өгөгдмөл нь `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Нэмэлт өгөгдмөл эзэмшигчийн токеныг бүх прокси хүсэлтэд ашигласан. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Дуудлага хийгчийн `Authorization` толгой хэсгийг үгчлэн дамжуулахын тулд `1` гэж тохируулна уу. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Санах ойн хурд хязгаарлагчийн тохиргоо (өгөгдмөл: 60 секунд тутамд 60 хүсэлт). |
| `TRYIT_PROXY_MAX_BODY` | Хүсэлтийн хамгийн их ачааллыг хүлээн зөвшөөрсөн (байт, анхдагч 1МиБ). |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii хүсэлтийн дээд урсгалын завсарлага (өгөгдмөл 10000 мс). |

Прокси нь:

- `GET /healthz` - бэлэн байдлыг шалгах.
- `/proxy/*` - зам болон асуулгын мөрийг хадгалсан прокси хүсэлтүүд.

## 3. Порталыг ажиллуул

Тусдаа терминал дээр:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` руу зочилж, Try It консолыг ашиглана уу. Үүнтэй адил
орчны хувьсагч нь Swagger UI болон RapiDoc суулгацыг тохируулдаг.

## 4. Нэгжийн туршилтуудыг ажиллуулж байна

Прокси нь зангилаанд суурилсан хурдан тестийн багцыг харуулж байна:

```bash
npm run test:tryit-proxy
```

Туршилтууд нь хаяг задлан шинжлэх, гарал үүслийн зохицуулалт, хувь хэмжээг хязгаарлах, эзэмшигчийг хамардаг
тарилга.

## 5. Шинжилгээний автоматжуулалт ба хэмжүүр

`/healthz` болон дээжийн төгсгөлийн цэгийг шалгахын тулд багцалсан датчикийг ашиглана уу:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Байгаль орчны товчлуурууд:

- `TRYIT_PROXY_SAMPLE_PATH` — дасгал хийх нэмэлт Torii маршрут (`/proxy` байхгүй).
- `TRYIT_PROXY_SAMPLE_METHOD` — анхдагч нь `GET`; бичих маршрутын хувьд `POST` гэж тохируулна.
- `TRYIT_PROXY_PROBE_TOKEN` — дээжийн дуудлагад түр зуурын тэмдэгтийг оруулна.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — өгөгдмөл 5 секундын хугацааг хүчингүй болгодог.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds`-д зориулсан Prometheus текст файлын очих газар.
- `TRYIT_PROXY_PROBE_LABELS` — хэмжигдэхүүнд хавсаргасан таслалаар тусгаарлагдсан `key=value` хосууд (`job=tryit-proxy` болон `instance=<proxy URL>`-ийн өгөгдмөл).

`TRYIT_PROXY_PROBE_METRICS_FILE` тохируулагдсан үед скрипт нь файлыг дахин бичдэг
атомын хувьд таны node_exporter/textfile цуглуулагч үргэлж бүрэн гүйцэд хардаг
ачаалал. Жишээ:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Үр дүнгийн хэмжигдэхүүнийг Prometheus руу шилжүүлж, жишээний дохиог дахин ашиглана уу.
`probe_success` `0` болж буурах үед хөгжүүлэгч-порталын баримтуудыг хуудас руу оруулна.

## 6. Үйлдвэрлэлийг хатууруулах хяналтын хуудас

Орон нутгийн хөгжлөөс гадна проксиг нийтлэхээс өмнө:

- TLS-ийг проксигийн өмнө дуусгах (урвуу прокси эсвэл удирддаг гарц).
- Бүтэцлэгдсэн бүртгэлийг тохируулж, ажиглалтын шугам руу шилжүүлэх.
- Тэмдэглэгээний токенуудыг эргүүлж, нууц менежертээ хадгалаарай.
- Проксины `/healthz` төгсгөлийн цэг болон нийт хоцрогдлын хэмжигдэхүүнийг хянах.
- Үнийн хязгаарлалтыг Torii шатлалын квоттой тааруулах; `Retry-After` тохируулна уу
  Үйлчлүүлэгчидтэй харилцах харилцааг багасгах зан үйл.