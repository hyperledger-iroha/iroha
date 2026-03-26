---
lang: mn
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Портал нь урсгалыг Torii руу дамжуулдаг гурван интерактив гадаргууг багцалсан:

- `/reference/torii-swagger` дээрх **Swagger UI** нь гарын үсэг зурсан OpenAPI үзүүлэлтийг гаргаж, `TRYIT_PROXY_PUBLIC_URL` тохируулагдсан үед проксигоор дамжуулан хүсэлтийг автоматаар дахин бичдэг.
- `/reference/torii-rapidoc` дээрх **RapiDoc** нь `application/x-norito`-д сайн ажилладаг файл байршуулах, контентын төрлийн сонгогчтой ижил схемийг харуулж байна.
- **Оролдоод үз дээ хамгаалагдсан хязгаарлагдмал орчинд** Norito тойм хуудас нь түр зуурын REST хүсэлт болон OAuth-төхөөрөмжөөр нэвтрэхэд хялбар маягтаар хангадаг.

Бүх гурван виджет нь орон нутгийн **Try-It proxy** (`docs/portal/scripts/tryit-proxy.mjs`) руу хүсэлт илгээдэг. Прокси нь `static/openapi/torii.json` нь `static/openapi/manifest.json`-д гарын үсэг зурсан тоймтой таарч байгаа эсэхийг шалгаж, хурд хязгаарлагчийг мөрдүүлж, `X-TryIt-Auth` гарчигуудыг логуудаар засварлаж, `X-TryIt-Client`-тэй дээд урсгалын дуудлага бүрийг тэмдэглэж, `X-TryIt-Client` эх сурвалжийг I0180 оператороор тэмдэглэдэг.

## Прокси ажиллуулна уу

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

- `TRYIT_PROXY_TARGET` бол таны дасгал хийхийг хүсч буй Torii үндсэн URL юм.
- `TRYIT_PROXY_ALLOWED_ORIGINS` нь консолыг оруулах ёстой портал гарал үүсэл бүрийг (орон нутгийн хөгжүүлэгч сервер, үйлдвэрлэлийн хостын нэр, урьдчилан үзэх URL) агуулсан байх ёстой.
- `TRYIT_PROXY_PUBLIC_URL`-г `docusaurus.config.js` ашиглаж, `customFields.tryIt`-ээр дамжуулан виджетүүдэд оруулна.
- `TRYIT_PROXY_BEARER` зөвхөн `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` үед ачаална; эс бөгөөс хэрэглэгчид консол эсвэл OAuth төхөөрөмжийн урсгалаар дамжуулан өөрсдийн токеныг нийлүүлэх ёстой.
- `TRYIT_PROXY_CLIENT_ID` нь хүсэлт болгонд авч явдаг `X-TryIt-Client` шошгыг тохируулдаг.
  Хөтөчөөс `X-TryIt-Client`-г нийлүүлэхийг зөвшөөрсөн боловч утгыг багасгасан
  хяналтын тэмдэгтүүдийг агуулж байвал татгалзана.

Эхлэх үед прокси нь `verifySpecDigest`-г ажиллуулж, манифест хуучирсан тохиолдолд засварын сануулсаар гарна. Хамгийн сүүлийн үеийн Torii тодорхойлолтыг татаж авахын тулд `npm run sync-openapi -- --latest`-г ажиллуул эсвэл яаралтай тусламжийн үед `TRYIT_PROXY_ALLOW_STALE_SPEC=1`-г дамжуулна уу.

Орчны файлуудыг гараар засварлахгүйгээр прокси зорилтот програмыг шинэчлэх эсвэл буцаахын тулд туслахыг ашиглана уу:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Виджетүүдийг холбоно уу

Прокси сонссоны дараа порталд үйлчлэх:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` дараах товчлууруудыг нээнэ:

| Хувьсагч | Зорилго |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL-г Swagger, RapiDoc болон Try it хамгаалагдсан хязгаарлагдмал орчинд оруулсан. Зөвшөөрөлгүй урьдчилан үзэх үед виджетүүдийг нуухын тулд тохируулаагүй орхино уу. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Санах ойд хадгалагдсан нэмэлт өгөгдмөл токен. Та `DOCS_SECURITY_ALLOW_INSECURE=1`-г дотооддоо нэвтрүүлэхгүй бол `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` болон зөвхөн HTTPS-н CSP хамгаалалт (DOCS-1b) шаардлагатай. |
| `DOCS_OAUTH_*` | OAuth төхөөрөмжийн урсгалыг (`OAuthDeviceLogin` бүрэлдэхүүн хэсэг) идэвхжүүлснээр тоймчид порталаас гаралгүйгээр богино хугацааны жетон гаргах боломжтой. |

OAuth хувьсагчууд байгаа үед хамгаалагдсан хязгаарлагдмал орчин нь тохируулсан Auth серверээр дамждаг **Төхөөрөмжийн кодоор нэвтрэх** товчийг харуулна (яг дүрсийг `config/security-helpers.js` харна уу). Төхөөрөмжийн урсгалаар дамжуулан гаргасан токенууд нь зөвхөн хөтчийн сессэд хадгалагдана.

## Norito-RPC ачааллыг илгээж байна

1. [Norito хурдан эхлүүлэх](./quickstart.md)-д тайлбарласан CLI эсвэл хэсгүүдийн тусламжтайгаар `.norito` ачааллыг үүсгэнэ үү. Прокси нь `application/x-norito`-ийн биетүүдийг өөрчлөгдөөгүй дамжуулдаг тул та `curl`-тэй нийтэлсэн ижил олдворыг дахин ашиглах боломжтой.
2. `/reference/torii-rapidoc` (хоёртын цэнэгийн хувьд илүүд үздэг) эсвэл `/reference/torii-swagger`-ийг нээнэ үү.
3. Унждаг цэснээс хүссэн Torii агшин зуурын зургийг сонгоно уу. Хормын хувилбаруудад гарын үсэг зурсан; самбар нь `static/openapi/manifest.json`-д бичигдсэн манифест дижестийг харуулж байна.
4. "Оролдоод үзээрэй" шүүгээнээс `application/x-norito` агуулгын төрлийг сонгоод **Файл сонгох** гэснийг товшоод ачаагаа сонгоно уу. Прокси нь хүсэлтийг `/proxy/v1/pipeline/submit` руу дахин бичиж, `X-TryIt-Client=docs-portal-rapidoc` гэж тэмдэглэнэ.
5. Norito хариултыг татахын тулд `Accept: application/x-norito`-г тохируулна уу. Swagger/RapiDoc нь толгойн сонгогчийг нэг шургуулганд гаргаж, хоёртын файлыг проксигоор дамжуулан буцааж цацна.

Зөвхөн JSON-д зориулсан чиглүүлэлтийн хувьд суулгагдсан Try it хамгаалагдсан хязгаарлагдмал орчин нь ихэвчлэн илүү хурдан байдаг: замыг (жишээ нь, `/v1/accounts/soraカタカナ.../assets`) оруулаад, HTTP аргыг сонгоод, шаардлагатай үед JSON-ийн үндсэн хэсгийг буулгаад, **Хүсэлт илгээх** дээр дарж толгой хэсэг, үргэлжлэх хугацаа, ачааллыг шугамаар шалгана уу.

## Алдааг олж засварлах

| Шинж тэмдэг | Болзошгүй шалтгаан | Засах |
| --- | --- | --- |
| Хөтчийн консол нь CORS алдааг харуулж байна эсвэл хамгаалагдсан орчин нь прокси URL байхгүй байгааг анхааруулж байна. | Прокси ажиллахгүй эсвэл эх сурвалж нь зөвшөөрөгдсөн жагсаалтад ороогүй байна. | Прокси эхлүүлж, `TRYIT_PROXY_ALLOWED_ORIGINS` таны портал хостыг хамарч байгаа эсэхийг шалгаад `npm run start`-г дахин эхлүүлнэ үү. |
| `npm run tryit-proxy` "дигест таарахгүй" гарна. | Torii OpenAPI багц нь урсгалын өмнө өөрчлөгдсөн. | `npm run sync-openapi -- --latest` (эсвэл `--version=<tag>`) ажиллуулаад дахин оролдоно уу. |
| Виджетүүд `401` эсвэл `403` буцаана. | Токен байхгүй, хугацаа нь дууссан эсвэл хамрах хүрээ хангалтгүй. | OAuth төхөөрөмжийн урсгалыг ашиглах эсвэл хамгаалагдсан хязгаарлагдмал орчинд хүчинтэй эзэмшигчийн токеныг буулгана уу. Статик токенуудын хувьд та `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`-г экспортлох ёстой. |
| Проксиас `429 Too Many Requests`. | IP-н тарифын хязгаар хэтэрсэн. | Итгэмжлэгдсэн орчин эсвэл тохируулагч тестийн скриптүүдийн хувьд `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS`-ийг өсгө. Бүх тарифын хязгаараас татгалзах `tryit_proxy_rate_limited_total` нэмэгдэнэ. |

## Ажиглалт

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs`-ийн эргэн тойрон дахь боодол) `/healthz` руу залгаж, сонголтоор жишээ маршрутыг дасгалжуулж, I18NI0000007X / I100700-д зориулсан Prometheus текст файлуудыг ялгаруулдаг. `TRYIT_PROXY_PROBE_METRICS_FILE`-г node_exporter-тэй нэгтгэхийн тулд тохируулна уу.
- Тоолуур (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) болон саатлын гистограммуудыг харуулахын тулд `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` тохируулна. `dashboards/grafana/docs_portal.json` самбар нь DOCS-SORA SLO-г хэрэгжүүлэхийн тулд эдгээр хэмжигдэхүүнийг уншдаг.
- Ажиллах цагийн бүртгэлүүд stdout дээр шууд гардаг. Оруулга бүрд хүсэлтийн id, дээд талын төлөв, баталгаажуулалтын эх сурвалж (`default`, `override`, эсвэл `client`) болон үргэлжлэх хугацаа орно; нууцыг ялгаруулахаас өмнө арилгадаг.

Хэрэв та `application/x-norito`-ийн ачаалал өөрчлөгдөөгүй Torii-д хүрч байгааг баталгаажуулах шаардлагатай бол Jest Suite-г (`npm test -- tryit-proxy`) ажиллуулах эсвэл `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`-ийн дагуу бэхэлгээг шалгана уу. Регрессийн тестүүд нь шахсан Norito хоёртын файлууд, гарын үсэг зурсан OpenAPI манифестууд болон проксигийн бууралтын замыг хамардаг тул NRPC нэвтрүүлэлт нь байнгын нотлох баримтыг үлдээдэг.