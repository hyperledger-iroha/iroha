---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Overview
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC тойм

Norito-RPC нь Torii API-д зориулсан хоёртын дамжуулалт юм. Энэ нь ижил HTTP замыг дахин ашигладаг
`/v1/pipeline` хэлбэрээр гэхдээ схемийг агуулсан Norito хүрээтэй ачааллыг солилцдог
хэш болон хяналтын нийлбэр. Тодорхой, баталгаатай хариулт хэрэгтэй үед үүнийг ашиглаарай
дамжуулах хоолойн JSON хариултууд саад болж хувирах үед.

## Яагаад солих вэ?
- CRC64 болон схемийн хэш бүхий тодорхойлогч фрейм нь код тайлах алдааг багасгадаг.
- SDK дээр хуваалцсан Norito туслахууд нь танд одоо байгаа өгөгдлийн загварын төрлийг дахин ашиглах боломжийг олгоно.
- Torii телеметрийн Norito сессийг аль хэдийн тэмдэглэсэн тул операторууд хянах боломжтой.
өгөгдсөн хяналтын самбараар үрчлүүлэх.

## Хүсэлт гаргаж байна

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. Ачаагаа Norito кодлогчоор (`iroha_client`, SDK туслахууд эсвэл
   `norito::to_bytes`).
2. Хүсэлтийг `Content-Type: application/x-norito`-ээр илгээнэ үү.
3. `Accept: application/x-norito` ашиглан Norito хариултыг хүсэх.
4. Тохирох SDK туслагчийг ашиглан хариултын кодыг тайл.

SDK-д зориулсан заавар:
- **Зэв**: Таныг тохируулах үед `iroha_client::Client` автоматаар Norito тохиролцоно.
  `Accept` толгой.
- **Python**: `iroha_python.norito_rpc`-аас `NoritoRpcClient`-г ашиглана уу.
- **Android**: `NoritoRpcClient` болон `NoritoRpcRequestOptions`-г ашиглана уу.
  Android SDK.
- **JavaScript/Swift**: туслагчдыг `docs/source/torii/norito_rpc_tracker.md` дээр хянадаг.
  мөн NRPC-3-ын нэг хэсэг болгон газардах болно.

## Консолын жишээг туршаад үзээрэй

Хөгжүүлэгчийн портал нь "Try It" прокси илгээдэг бөгөөд ингэснээр шүүмжлэгчид Norito-г дахин тоглуулах боломжтой.
захиалгат скрипт бичихгүйгээр ачаалал.

1. [Прокси эхлүүлнэ](./try-it.md#start-the-proxy-locally) ба тохируулна уу
   `TRYIT_PROXY_PUBLIC_URL`, ингэснээр виджетүүд хаашаа траффик илгээхээ мэддэг.
2. Энэ хуудсан дээрх **Оролдоод үз** карт эсвэл `/reference/torii-swagger`-г нээнэ үү.
   самбар болон `POST /v1/pipeline/submit` гэх мэт төгсгөлийн цэгийг сонгоно уу.
3. **Агуулгын төрөл**-г `application/x-norito` болгож, **Хоёртын файл**-г сонгоно уу.
   засварлагч болон `fixtures/norito_rpc/transfer_asset.norito` байршуулна уу
   (эсвэл жагсаалтад орсон аливаа ачаа
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. OAuth төхөөрөмжийн кодын виджет эсвэл гарын авлагын токеноор дамжуулан эзэмшигчийн токеныг өгөх
   талбар (прокси нь тохируулсан үед `X-TryIt-Auth`-г хүчингүй болгодог
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Хүсэлтийг илгээж, Torii нь жагсаалтад орсон `schema_hash`-тэй нийцэж байгаа эсэхийг шалгаарай.
   `fixtures/norito_rpc/schema_hashes.json`. Тохиромжтой хэшүүд нь
   Norito толгой нь хөтөч/прокси хопоос хамгаалагдсан.

Замын зургийн нотлох баримтыг авахын тулд Try It дэлгэцийн агшинг гүйлттэй хослуулна уу
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Скриптийг боож байна
`cargo xtask norito-rpc-verify` гэж JSON хураангуйг бичнэ
`artifacts/norito_rpc/<timestamp>/`, мөн адил бэхэлгээг барьж авдаг
порталыг ашигласан.

## Алдааг олж засварлах

| Шинж тэмдэг | Хаана харагдаж байна | Болзошгүй шалтгаан | засах |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii хариу | Дутуу эсвэл буруу `Content-Type` толгой | Ачаа илгээхийн өмнө `Content-Type: application/x-norito`-г тохируулна уу. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii хариултын хэсэг/толгой | Бэхэлгээний схемийн хэш нь Torii загвараас ялгаатай | `cargo xtask norito-rpc-fixtures` ашиглан бэхэлгээг сэргээж, `fixtures/norito_rpc/schema_hashes.json` дахь хэшийг баталгаажуулна уу; Хэрэв төгсгөлийн цэг Norito-г идэвхжүүлээгүй бол JSON руу буцна уу. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Оролдоод үзээрэй прокси хариу | Хүсэлт нь `TRYIT_PROXY_ALLOWED_ORIGINS` | жагсаалтад ороогүй эх сурвалжаас ирсэн Env var-д портал эх үүсвэрийг (жишээ нь, `https://docs.devnet.sora.example`) нэмээд проксиг дахин эхлүүлнэ үү. |
| `{"error":"rate_limited"}` (HTTP 429) | Оролдоод үзээрэй прокси хариу | IP тутамд квот `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` төсвөөс хэтэрсэн | Дотоод ачааллын туршилтын хязгаарыг нэмэгдүүлэх эсвэл цонхыг дахин тохируулах хүртэл хүлээнэ үү (JSON хариултын `retryAfterMs` хэсгийг үзнэ үү). |
| `{"error":"upstream_timeout"}` (HTTP 504) эсвэл `{"error":"upstream_error"}` (HTTP 502) | Оролдоод үзээрэй прокси хариу | Torii хугацаа хэтэрсэн эсвэл прокси тохируулсан арын хэсэгт хүрч чадсангүй | `TRYIT_PROXY_TARGET`-д холбогдох боломжтой эсэхийг шалгах, Torii-ийн эрүүл мэндийг шалгах эсвэл илүү том `TRYIT_PROXY_TIMEOUT_MS` ашиглан дахин оролдоно уу. |

Илүү ихийг туршиж үзээрэй оношлогоо болон OAuth зөвлөмжүүд байдаг
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Нэмэлт нөөц
- Тээврийн RFC: `docs/source/torii/norito_rpc.md`
- Гүйцэтгэх хураангуй: `docs/source/torii/norito_rpc_brief.md`
- Үйлдлийн хянагч: `docs/source/torii/norito_rpc_tracker.md`
- Туршиж үзээрэй прокси заавар: `docs/portal/docs/devportal/try-it.md`