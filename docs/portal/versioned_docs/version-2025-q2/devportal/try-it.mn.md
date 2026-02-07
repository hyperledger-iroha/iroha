---
lang: mn
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Хамгаалалтын орчинд туршаад үзээрэй

Хөгжүүлэгчийн портал нь нэмэлт "Оролдоод үзээрэй" консолыг нийлүүлдэг тул та Torii руу залгах боломжтой.
баримт бичгийг орхихгүйгээр төгсгөлийн цэгүүд. Консол нь хүсэлтийг дамжуулдаг
багцын прокси ашиглан хөтчүүд CORS хязгаарыг давж гарах боломжтой
хурдны хязгаарлалт болон баталгаажуулалтыг хэрэгжүүлэх.

## Урьдчилсан нөхцөл

- Node.js 18.18 буюу түүнээс дээш хувилбар (портал бүтээх шаардлагад нийцдэг)
- Torii шатлалын орчинд сүлжээнд нэвтрэх
- Таны дасгал хийхээр төлөвлөж буй Torii маршрутуудыг дуудах боломжтой зөөгч токен

Бүх прокси тохиргоог орчны хувьсагчаар гүйцэтгэдэг. Доорх хүснэгт
Хамгийн чухал товчлууруудыг жагсаав:

| Хувьсагч | Зорилго | Өгөгдмөл |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Прокси нь | руу хүсэлт илгээдэг Torii үндсэн URL **Шаардлагатай** |
| `TRYIT_PROXY_LISTEN` | Орон нутгийн хөгжлийн хаягийг сонсох (`host:port` эсвэл `[ipv6]:port` формат) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Проксиг дуудаж болох гарал үүслийн таслалаар тусгаарлагдсан жагсаалт | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | Өгөгдмөл эзэмшигчийн токеныг Torii | руу шилжүүлсэн _хоосон_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Эцсийн хэрэглэгчдэд `X-TryIt-Auth` |-ээр дамжуулан өөрийн жетон нийлүүлэхийг зөвшөөрнө үү `0` |
| `TRYIT_PROXY_MAX_BODY` | Хүсэлтийн дээд хэмжээ (байт) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Урсгалын өмнөх хугацаа миллисекундээр | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Үйлчлүүлэгчийн IP-д ногдох тарифын цонхонд зөвшөөрөгдсөн хүсэлт | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Хурд хязгаарлах гүйдэг цонх (мс) | `60000` |

Прокси нь мөн `GET /healthz`-г илрүүлж, бүтэцлэгдсэн JSON алдааг буцаана.
бүртгэлийн гаралтаас зөөгч жетоныг засварлана.

## Проксиг дотоодоос эхлүүлнэ үү

Порталыг анх тохируулахдаа хамаарлыг суулгана уу:

```bash
cd docs/portal
npm install
```

Прокси ажиллуулаад Torii жишээ рүү чиглүүл:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Скрипт нь холбогдох хаягийг бүртгэж, хүсэлтийг `/proxy/*`-с дамжуулдаг.
тохируулагдсан Torii гарал үүсэл.

## Порталын виджетүүдийг холбоно уу

Та хөгжүүлэгчийн порталыг бүтээх эсвэл үйлчлэх үедээ виджетүүдийн URL-ыг тохируулна уу
проксид ашиглах ёстой:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Дараах бүрэлдэхүүн хэсгүүд нь `docusaurus.config.js`-аас эдгээр утгыг уншина:

- **Swagger UI** — `/reference/torii-swagger` дээр үзүүлсэн; хүсэлтийг ашигладаг
  эзэмшигчийн токенуудыг автоматаар хавсаргах interceptor.
- **RapiDoc** — `/reference/torii-rapidoc` дээр үзүүлсэн; токен талбарыг толин тусгал
  мөн проксигийн эсрэг оролдох хүсэлтийг дэмждэг.
- **Консолыг туршаад үзээрэй** — API тойм хуудсанд суулгагдсан; захиалгаар илгээх боломжийг танд олгоно
  хүсэлт, толгой хэсгийг харах, хариулах хэсгүүдийг шалгах.

Аливаа виджет дээрх токеныг өөрчлөх нь зөвхөн одоогийн хөтчийн сессэд нөлөөлнө; нь
прокси нь нийлүүлсэн токеныг хэзээ ч хадгалахгүй эсвэл бүртгэдэггүй.

## Ажиглалт ба үйл ажиллагаа

Хүсэлт бүрийг арга, зам, гарал үүсэл, дээд урсгалын төлөв болон
баталгаажуулалтын эх сурвалж (`override`, `default`, эсвэл `client`). Токенууд хэзээ ч байдаггүй
хадгалагдсан—ашиглагчийн толгой болон `X-TryIt-Auth` утгуудыг өмнө нь засварласан
мод бэлтгэх—тэгэхээр та stdout-ийг төв коллектор руу санаа зовохгүйгээр дамжуулах боломжтой
нууц задрах.

### Эрүүл мэндийн үзлэг ба анхааруулгаБайрлуулалтын үеэр эсвэл хуваарийн дагуу багцалсан датчикийг ажиллуулна уу:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Байгаль орчны товчлуурууд:

- `TRYIT_PROXY_SAMPLE_PATH` — дасгал хийх нэмэлт Torii маршрут (`/proxy`-гүй).
- `TRYIT_PROXY_SAMPLE_METHOD` — анхдагч нь `GET`; бичих маршрутын хувьд `POST` гэж тохируулна.
- `TRYIT_PROXY_PROBE_TOKEN` — жишээ дуудлагын түр зуурын тэмдэгтийг оруулна.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — өгөгдмөл 5 секундын хугацааг хүчингүй болгодог.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds`-д зориулсан нэмэлт Prometheus текст файлын очих газар.
- `TRYIT_PROXY_PROBE_LABELS` — таслалаар тусгаарлагдсан `key=value` хосууд хэмжигдэхүүнд хавсаргасан (`job=tryit-proxy` болон `instance=<proxy URL>`-ийн өгөгдмөл).

Пробыг бичиж болохуйц руу чиглүүлэх замаар үр дүнг текст файл цуглуулагч руу оруулна
зам (жишээ нь, `/var/lib/node_exporter/textfile_collector/tryit.prom`) болон
дурын шошго нэмэх:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

Скрипт нь хэмжүүрийн файлыг атомаар дахин бичдэг тул таны цуглуулагч үргэлж a
бүрэн ачаалал.

Хөнгөн дохио өгөхийн тулд датчикийг хяналтын стек рүүгээ холбоно уу. A Prometheus
хоёр дараалсан алдааны дараах хуудасны жишээ:

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

### Буцах автоматжуулалт

Зорилтот Torii URL-г шинэчлэх эсвэл сэргээхийн тулд удирдлагын туслахыг ашиглана уу. Скрипт
`.env.tryit-proxy.bak`-д өмнөх тохиргоог хадгалдаг тул буцаалт нь
ганц тушаал.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Env файлын замыг `--env` эсвэл `TRYIT_PROXY_ENV` ашиглан дарж бичнэ үү.
тохиргоог өөр газар хадгалдаг.