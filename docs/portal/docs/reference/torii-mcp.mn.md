<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii MCP API
description: Torii-ийн эх загвар контекст протоколын гүүрийг ашиглах лавлах гарын авлага.
---

Torii нь `/v1/mcp` дээр эх загвар контекст протокол (MCP) гүүрийг харуулж байна.
Энэ төгсгөлийн цэг нь агентуудад хэрэгслүүдийг олж илрүүлэх, JSON-RPC-ээр дамжуулан Torii/Connect чиглүүлэлтүүдийг дуудах боломжийг олгодог.

## Төгсгөлийн цэгийн хэлбэр

- `GET /v1/mcp` чадварын мета өгөгдлийг буцаана (JSON-RPC ороосон биш).
- `POST /v1/mcp` JSON-RPC 2.0 хүсэлтийг хүлээн авдаг.
- Хэрэв `torii.mcp.enabled = false` бол аль ч маршрут ил гарахгүй.
- Хэрэв `torii.require_api_token` идэвхжсэн бол JSON-RPC илгээхээс өмнө дутуу/хүчингүй токен татгалзана.

## Тохиргоо

`torii.mcp` дор MCP-г идэвхжүүлэх:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Гол зан үйл:

- `profile` багажны харагдах байдлыг хянадаг (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` нэмэлт нэр дээр суурилсан бодлогыг хэрэгжүүлнэ.
- `rate_per_minute`/`burst` MCP хүсэлтийн хувьд токен хувин хязгаарлалтыг хэрэглэнэ.
- `tools/call_async`-ийн асинхронгүй ажлын төлөвийг `async_job_ttl_secs` болон `async_job_max_entries` ашиглан санах ойд хадгалдаг.

## Санал болгож буй үйлчлүүлэгчийн урсгал

1. `initialize` руу залгаарай.
2. `tools/list` руу залгаад `toolsetVersion` кэш.
3. Ердийн үйл ажиллагаанд `tools/call` ашиглана уу.
4. Удаан ажиллахын тулд `tools/call_async` + `tools/jobs/get` ашиглана уу.
5. `listChanged` `true` үед `tools/list`-г дахин ажиллуул.

Хэрэгслийн бүрэн каталогийг хатуу кодлох хэрэггүй. Ажиллах үед олж мэдээрэй.

## Арга зүй ба семантик

Дэмжигдсэн JSON-RPC аргууд:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Тэмдэглэл:- `tools/list` нь `toolset_version` болон `toolsetVersion` хоёуланг нь хүлээн авдаг.
- `tools/jobs/get` нь `job_id` болон `jobId` хоёуланг нь хүлээн авдаг.
- `tools/list.cursor` нь тоон мөрийн офсет; хүчингүй утгууд `0` руу буцна.
- `tools/call_batch` нь нэг зүйл бүрт хамгийн сайн хүчин чармайлт юм (нэг бүтэлгүйтсэн дуудлага ах дүүгийн дуудлагад амжилтгүй болдоггүй).
- `tools/call_async` зөвхөн дугтуйны хэлбэрийг нэн даруй баталгаажуулна; гүйцэтгэлийн алдаа нь ажлын төлөвт хожуу гарч ирдэг.
- `jsonrpc` `"2.0"` байх ёстой; орхигдуулсан `jsonrpc` нийцтэй байхаар хүлээн зөвшөөрөгдсөн.

## Зөвшөөрөл болон дамжуулалт

MCP диспетч нь Torii зөвшөөрлийг тойрч гарахгүй. Дуудлага нь ердийн чиглүүлэлтийн зохицуулагч болон баталгаажуулалтын шалгалтыг гүйцэтгэдэг.

Torii багажийн илгээлтийн дотогшоо баталгаажуулалттай холбоотой толгой хэсгийг дамжуулдаг:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Үйлчлүүлэгчид мөн `arguments.headers`-ээр дамжуулан дуудлага тус бүрийн нэмэлт гарчиг өгөх боломжтой.
`content-length`, `host`, `arguments.headers`-аас `connection`, үл тоомсорлодог.

## Алдааны загвар

HTTP давхарга:

- `400` хүчингүй JSON
- JSON-RPC-тэй ажиллахаас өмнө `403` API токен татгалзсан
- `413` ачааллын хэмжээ `max_request_bytes`-ээс хэтэрсэн.
- `429` хурдны хязгаартай
- JSON-RPC хариултуудад зориулсан `200` (JSON-RPC алдааг оруулаад)

JSON-RPC давхарга:- Дээд түвшний `error.data.error_code` тогтвортой байна (жишээ нь `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I00303).
- Багажны эвдрэл нь MCP хэрэгслийн үр дүнд `isError = true` болон бүтэцлэгдсэн дэлгэрэнгүй мэдээлэлтэй гарч ирдэг.
- Маршрутаар илгээсэн хэрэгслийн алдаа нь HTTP статусыг `structuredContent.error_code` (жишээ нь `forbidden`, `not_found`, `server_error`) дээр буулгана.

## Хэрэгслийн нэршил

OpenAPI-аас гаралтай хэрэгслүүд нь маршрутад суурилсан тогтвортой нэрийг ашигладаг:

- `torii.<method>_<path...>`
- Жишээ нь: `torii.get_v1_accounts`

Мөн `iroha.*` болон `connect.*` доор сонгогдсон бусад нэрс гарч ирдэг.

## Каноник тодорхойлолт

Бүрэн утсан түвшний гэрээг дараах байдлаар хадгална:

- `crates/iroha_torii/docs/mcp_api.md`

`crates/iroha_torii/src/mcp.rs` эсвэл `crates/iroha_torii/src/lib.rs`-д зан төлөв өөрчлөгдөх үед,
ижил өөрчлөлтөөр тухайн техникийн үзүүлэлтийг шинэчилж, дараа нь түлхүүрийн ашиглалтын удирдамжийг энд тусгана уу.