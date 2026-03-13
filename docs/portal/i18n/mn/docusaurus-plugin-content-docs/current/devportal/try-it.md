---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Хамгаалалтын орчинд туршаад үзээрэй

Хөгжүүлэгчийн портал нь нэмэлт "Оролдоод үзээрэй" консолыг нийлүүлдэг тул та Torii руу залгах боломжтой.
баримт бичгийг орхихгүйгээр төгсгөлийн цэгүүд. Консол нь хүсэлтийг дамжуулдаг
багцын прокси ашиглан хөтчүүд CORS хязгаарыг давж гарах боломжтой
хурдны хязгаарлалт болон баталгаажуулалтыг хэрэгжүүлэх.

## Урьдчилсан нөхцөл

- Node.js 18.18 буюу түүнээс дээш хувилбар (портал бүтээх шаардлагад нийцдэг)
- Torii шатлалын орчинд сүлжээний хандалт
- Таны дасгал хийхээр төлөвлөж буй Torii маршрутыг дуудах боломжтой зөөгч токен

Бүх прокси тохиргоог орчны хувьсагчаар гүйцэтгэдэг. Доорх хүснэгт
Хамгийн чухал товчлууруудыг жагсаав:

| Хувьсагч | Зорилго | Өгөгдмөл |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Прокси нь | руу хүсэлт илгээдэг үндсэн Torii URL **Шаардлагатай** |
| `TRYIT_PROXY_LISTEN` | Орон нутгийн хөгжлийн хаягийг сонсох (`host:port` эсвэл `[ipv6]:port` формат) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Проксиг дуудаж болох гарал үүслийн таслалаар тусгаарлагдсан жагсаалт | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Дээд талын хүсэлт болгонд `X-TryIt-Client` дээр байрлуулсан танигч | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Өгөгдмөл эзэмшигчийн токеныг Torii | руу шилжүүлсэн _хоосон_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Эцсийн хэрэглэгчдэд `X-TryIt-Auth` |-ээр дамжуулан өөрийн жетон нийлүүлэхийг зөвшөөрнө үү `0` |
| `TRYIT_PROXY_MAX_BODY` | Хүсэлтийн дээд хэмжээ (байт) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Урсгалын өмнөх хугацаа миллисекундээр | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Үйлчлүүлэгчийн IP-д ногдох тарифын цонхонд зөвшөөрөгдсөн хүсэлт | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Хурд хязгаарлах гүйдэг цонх (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus загварын хэмжүүрийн төгсгөлийн цэгийн (`host:port` эсвэл `[ipv6]:port`) нэмэлт сонсох хаяг | _хоосон (идэвхгүй)_ |
| `TRYIT_PROXY_METRICS_PATH` | Метрийн төгсгөлийн цэгээр үйлчилдэг HTTP зам | `/metrics` |

Прокси нь мөн `GET /healthz`-г илрүүлж, бүтэцлэгдсэн JSON алдааг буцаана.
бүртгэлийн гаралтаас зөөгч жетоныг засварлана.

Proxy-г docs хэрэглэгчдэд үзүүлэх үед `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`-г идэвхжүүлснээр Swagger болон
RapiDoc самбар нь хэрэглэгчээс нийлүүлсэн үүргийн токенуудыг дамжуулах боломжтой. Прокси нь тарифын хязгаарлалтыг мөрддөг хэвээр байна.
итгэмжлэлүүдийг засварлаж, хүсэлт нь өгөгдмөл жетон эсвэл хүсэлт бүрийг хүчингүй болгосон эсэхийг бүртгэдэг.
`TRYIT_PROXY_CLIENT_ID`-г `X-TryIt-Client` гэж илгээхийг хүссэн шошгондоо тохируулна уу
(өгөгдмөл нь `docs-portal`). Прокси нь дуудагчаас ирүүлсэн мэдээллийг тайрч, баталгаажуулдаг
`X-TryIt-Client` утгууд нь энэ өгөгдмөл рүү буцсан тул үе шатны гарцууд
хөтчийн мета өгөгдлийн хамааралгүйгээр гарал үүслийг аудит.

## Проксиг дотоодоос эхлүүлнэ үү

Порталыг анх тохируулахдаа хамаарлыг суулгана уу:

```bash
cd docs/portal
npm install
```

Прокси ажиллуулаад Torii жишээ рүү чиглүүлээрэй:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Скрипт нь холбогдох хаягийг бүртгэж, хүсэлтийг `/proxy/*`-аас
тохируулагдсан Torii гарал үүсэл.

Сокетыг холбохоос өмнө скрипт үүнийг баталгаажуулдаг
`static/openapi/torii.json`-д бүртгэгдсэн тайжид таарч байна
`static/openapi/manifest.json`. Хэрэв файлууд зөрөх юм бол тушаал нь гарч ирнэ
алдаа гаргаж, танд `npm run sync-openapi -- --latest`-г ажиллуулахыг заадаг. Экспорт
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` зөвхөн яаралтай тусламжийн үед; прокси болно
анхааруулга бүртгүүлж, үргэлжлүүлэхийн тулд засвар үйлчилгээний цонхны үеэр сэргээх боломжтой.

## Порталын виджетүүдийг холбоно уу

Та хөгжүүлэгчийн порталыг бүтээх эсвэл үйлчлэх үедээ виджетүүдийн URL-ыг тохируулна уу
проксид ашиглах ёстой:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Дараах бүрэлдэхүүн хэсгүүд нь `docusaurus.config.js`-ээс эдгээр утгыг уншина:

- **Swagger UI** — `/reference/torii-swagger` дээр үзүүлсэн; урьдчилан зөвшөөрөл олгодог
  Токен байгаа үед эзэмшигчийн схем, `X-TryIt-Client` бүхий хаягийн хүсэлт,
  `X-TryIt-Auth`-г тарьж, проксигоор дамжуулан дуудлагыг дахин бичдэг.
  `TRYIT_PROXY_PUBLIC_URL` тохируулагдсан.
- **RapiDoc** — `/reference/torii-rapidoc` дээр үзүүлсэн; токен талбарыг толин тусгал,
  Swagger самбартай ижил толгойг дахин ашиглаж, прокси руу чиглүүлдэг
  URL-г тохируулах үед автоматаар.
- **Консолыг туршаад үзээрэй** — API тойм хуудсанд суулгагдсан; захиалгаар илгээх боломжийг танд олгоно
  хүсэлт, толгой хэсгийг харах, хариулах хэсгүүдийг шалгах.

Хоёр самбар дээр уншдаг **агшин зуурын сонгогч** гарч ирнэ
`docs/portal/static/openapi/versions.json`. Энэ индексийг бөглөнө үү
`npm run sync-openapi -- --version=<label> --mirror=current --latest` тийм
Шүүгчид түүхэн үзүүлэлтүүдийн хооронд шилжих боломжтой, бүртгэгдсэн SHA-256 тоймыг үзэх,
мөн ашиглахаасаа өмнө хувилбарын агшин агшинд гарын үсэг зурсан манифест байгаа эсэхийг баталгаажуулна уу
интерактив виджетүүд.

Аливаа виджет дээрх токеныг өөрчлөх нь зөвхөн одоогийн хөтчийн сессэд нөлөөлнө; нь
прокси нь нийлүүлсэн токеныг хэзээ ч хадгалахгүй эсвэл бүртгэдэггүй.

## Богино хугацааны OAuth жетонууд

Шүүмжлэгчдэд удаан эдэлгээтэй Torii жетоныг тараахаас зайлсхийхийн тулд Try it утсаар холбогдоно уу.
консолыг өөрийн OAuth сервер рүү оруулна. Доорх орчны хувьсагч байгаа үед
портал нь төхөөрөмжийн кодын нэвтрэх виджетийг үзүүлж, богино хугацааны эзэмшигчийн жетонуудыг гаргаж,
мөн автоматаар консол хэлбэрт оруулдаг.

| Хувьсагч | Зорилго | Өгөгдмөл |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Төхөөрөмжийн зөвшөөрлийн төгсгөлийн цэг (`/oauth/device/code`) | _хоосон (идэвхгүй)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` | хүлээн авах токен төгсгөлийн цэг _хоосон_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth үйлчлүүлэгчийн таниулбарыг баримтыг урьдчилан үзэхэд бүртгүүлсэн | _хоосон_ |
| `DOCS_OAUTH_SCOPE` | Нэвтрэх үед зайгаар тусгаарлагдсан хамрах хүрээг хүссэн | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Токеныг | руу холбох нэмэлт API үзэгчид _хоосон_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Зөвшөөрөл хүлээж байх үеийн санал асуулгын хамгийн бага интервал (мс) | `5000` (<5000ms-ээс татгалзсан) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Буцах төхөөрөмж-кодын хугацаа дуусах цонх (секунд) | `600` (300-аас 900-н хооронд байх ёстой) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Буцах хандалтын токены ашиглалтын хугацаа (секунд) | `900` (300-аас 900-н хооронд байх ёстой) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth хэрэгжилтийг зориудаар алгасах локал урьдчилан үзэхийн тулд `1` гэж тохируулна уу | _тохируулаагүй_ |

Жишээ тохиргоо:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Таныг `npm run start` эсвэл `npm run build` ажиллуулах үед портал эдгээр утгыг оруулдаг.
`docusaurus.config.js` дээр. Орон нутгийн урьдчилан үзэх үед "Оролдоод үзээрэй" карт нь a харагдана
"Төхөөрөмжийн кодоор нэвтрэх" товч. Хэрэглэгчид таны OAuth дээр харуулсан кодыг оруулна
баталгаажуулах хуудас; төхөөрөмжийн урсгал виджетийг амжилттай гүйцэтгэсний дараа:

- "Try it console" талбарт олгосон эзэмшигчийн токеныг оруулна,
- одоо байгаа `X-TryIt-Client` болон `X-TryIt-Auth` гарчигтай хүсэлтийг шошголох,
- үлдсэн ашиглалтын хугацааг харуулна, мөн
- хугацаа дуусахад жетоныг автоматаар арилгана.

Гараар Bearer оруулах боломжтой хэвээр байна - OAuth хувьсагчийг хэзээ ч орхи
хянагчдыг түр зуурын жетоныг өөрсдөө буулгах эсвэл экспортлохыг албадахыг хүсч байна
`DOCS_OAUTH_ALLOW_INSECURE=1` нь нууцлагдсан орон нутгийн урьдчилан харах боломжтой
хүлээн зөвшөөрөх боломжтой. OAuth-г тохируулаагүй бүтээн байгуулалтууд одоо шаардлагыг хангаж чадахгүй байна
DOCS-1b замын зураглалын хаалга.

📌 [Аюулгүй байдлын хатуужилт ба үзэг шалгах хяналтын хуудас](./security-hardening.md)-г шалгана уу.
лабораторийн гадна порталыг ил гаргахаас өмнө; энэ нь аюул заналын загварыг баримтжуулж,
CSP/Итгэмжлэгдсэн төрлүүдийн профайл болон DOCS-1b-д нэвтэрч буй нэвтрэлтийн туршилтын алхамууд.

## Norito-RPC дээж

Norito-RPC хүсэлтүүд нь JSON маршруттай ижил прокси болон OAuth сантехникийг хуваалцдаг.
тэд зүгээр л `Content-Type: application/x-norito`-г тохируулаад илгээнэ үү
NRPC тодорхойлолтод тодорхойлсон урьдчилан кодлогдсон Norito ачааллыг
(`docs/source/torii/nrpc_spec.md`).
Хадгалах газар нь `fixtures/norito_rpc/` дор каноник ачааллыг илгээдэг тул портал
Зохиогчид, SDK эзэмшигчид болон тоймчид CI-ийн ашигладаг яг байтыг дахин тоглуулах боломжтой.

### Try It консолоос Norito ачааг илгээх

1. `fixtures/norito_rpc/transfer_asset.norito` гэх мэт бэхэлгээг сонго. Эдгээр
   файлууд нь түүхий Norito дугтуй; base64-ээр кодлох хэрэггүй.
2. Swagger эсвэл RapiDoc дээр NRPC төгсгөлийн цэгийг олоорой (жишээ нь
   `POST /v2/pipeline/submit`) ба **Агуулгын төрөл** сонгогчийг
   `application/x-norito`.
3. Хүсэлтийн үндсэн засварлагчийг **хоёртын файл руу шилжүүлнэ үү (Swagger-ийн "Файл" горим эсвэл
   RapiDoc-ийн "Хоёртын/Файл" сонгогч) болон `.norito` файлыг байршуулна уу. Виджет
   проксигоор дамжуулан байтуудыг ямар ч өөрчлөлтгүйгээр дамжуулдаг.
4. Хүсэлтийг илгээнэ үү. Хэрэв Torii нь `X-Iroha-Error-Code: schema_mismatch`-г буцаана.
   хоёртын ачааллыг хүлээн авдаг төгсгөлийн цэг рүү залгаж байгаа эсэхээ шалгаарай
   `fixtures/norito_rpc/schema_hashes.json`-д бичигдсэн схемийн хэшийг баталгаажуулна уу
   таны цохиж буй Torii загвартай таарч байна.

Консол нь хамгийн сүүлийн үеийн файлыг санах ойд хадгалдаг тул та үүнийг дахин илгээх боломжтой
өөр зөвшөөрлийн токенууд эсвэл Torii хостуудыг ашиглаж байх үед ачаалал. Нэмэх
Таны ажлын урсгал руу `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` үйлдвэрлэдэг
NRPC-4 батлах төлөвлөгөөнд дурдсан нотлох баримтын багц (лог + JSON хураангуй),
Энэ нь үнэлгээний үеэр "Оролдоод үзээрэй" гэсэн хариултын дэлгэцийн агшинд маш сайн нийцдэг.

### CLI жишээ (curl)

Ижил бэхэлгээг `curl`-ээр дамжуулан порталаас гадуур дахин тоглуулах боломжтой бөгөөд энэ нь ашигтай.
проксиг баталгаажуулах эсвэл гарцын хариултыг дибаг хийх үед:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json`-д жагсаасан дурын оруулгатай бэхэлгээг солино уу
эсвэл `cargo xtask norito-rpc-fixtures` ашиглан өөрийн ачааллыг кодчил. Torii үед
canary горимд байгаа бол та `curl`-г оролдох прокси дээр зааж болно
(`https://docs.sora.example/proxy/v2/pipeline/submit`) ижил дасгал хийх
портал виджетүүдийн ашигладаг дэд бүтэц.

## Ажиглалт ба үйл ажиллагааХүсэлт бүрийг арга, зам, гарал үүсэл, дээд урсгалын төлөв болон
баталгаажуулалтын эх сурвалж (`override`, `default`, эсвэл `client`). Токенууд хэзээ ч байдаггүй
хадгалагдсан—ашиглагчийн толгой болон `X-TryIt-Auth` утгуудыг өмнө нь засварласан
мод бэлтгэх—тэгэхээр та stdout-ийг төв коллектор руу санаа зовохгүйгээр дамжуулах боломжтой
нууц задарсан.

### Эрүүл мэндийн үзлэг ба анхааруулга

Байрлуулалтын үеэр эсвэл хуваарийн дагуу багцалсан датчикийг ажиллуулна уу:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Байгаль орчны товчлуурууд:

- `TRYIT_PROXY_SAMPLE_PATH` — дасгал хийх нэмэлт Torii маршрут (`/proxy`-гүй).
- `TRYIT_PROXY_SAMPLE_METHOD` — анхдагч нь `GET`; бичих маршрутын хувьд `POST` гэж тохируулна.
- `TRYIT_PROXY_PROBE_TOKEN` — дээжийн дуудлагад түр зуурын тэмдэгтийг оруулна.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — өгөгдмөл 5 секундын хугацааг хүчингүй болгодог.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds`-д зориулсан нэмэлт Prometheus текст файлын очих газар.
- `TRYIT_PROXY_PROBE_LABELS` — хэмжигдэхүүнд хавсаргасан таслалаар тусгаарлагдсан `key=value` хосууд (`job=tryit-proxy` ба `instance=<proxy URL>`-ийн өгөгдмөл).
- `TRYIT_PROXY_PROBE_METRICS_URL` — `TRYIT_PROXY_METRICS_LISTEN` идэвхжсэн үед амжилттай хариу өгөх шаардлагатай төгсгөлийн цэгийн URL (жишээ нь, I18NI0000142X).

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

`TRYIT_PROXY_METRICS_LISTEN` тохируулагдсан үед тохируулна уу
`TRYIT_PROXY_PROBE_METRICS_URL` хэмжүүрийн төгсгөлийн цэг рүү шилжүүлснээр датчик хурдан бүтэлгүйтдэг
хуссан гадаргуу алга бол (жишээлбэл, буруу тохируулагдсан оролт эсвэл байхгүй
галт ханын дүрэм). Ердийн үйлдвэрлэлийн тохиргоо нь
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

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

### Метрикийн төгсгөлийн цэг ба хяналтын самбар

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (эсвэл дурын хост/порт хос)-ыг өмнө нь тохируулна уу
Prometheus форматтай хэмжүүрийн төгсгөлийн цэгийг харуулах проксиг эхлүүлж байна. Зам
өгөгдмөл нь `/metrics` боловч дамжуулан дарж болно
`TRYIT_PROXY_METRICS_PATH=/custom`. хусах бүр аргын тоологчийг буцаана
хүсэлтийн нийлбэр, хувь хэмжээг хязгаарлахаас татгалзах, өмнөх алдаа/хугацаа, прокси үр дүн,
болон хоцрогдлын хураангуй:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP цуглуулагчаа хэмжүүрийн төгсгөлд чиглүүлж,
одоо байгаа `dashboards/grafana/docs_portal.json` хавтангууд нь SRE нь сүүлийг ажиглах боломжтой
логуудыг задлан шинжилдэггүй хоцрогдол болон татгалзсан өсөлт. Прокси автоматаар
операторуудад дахин эхлүүлэхийг илрүүлэхэд туслах зорилгоор `tryit_proxy_start_timestamp_ms`-ийг нийтэлдэг.

### Буцах автоматжуулалт

Зорилтот Torii URL-г шинэчлэх эсвэл сэргээхийн тулд удирдлагын туслахыг ашиглана уу. Скрипт
өмнөх тохиргоог `.env.tryit-proxy.bak`-д хадгалдаг тул буцаалт нь
ганц тушаал.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Env файлын замыг `--env` эсвэл `TRYIT_PROXY_ENV` ашиглан дарж бичнэ үү.
тохиргоог өөр газар хадгалдаг.