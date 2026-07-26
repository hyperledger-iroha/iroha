---
id: developer-sdk-index
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

SoraFS хэрэгслийн гинжтэй хамт ирдэг хэл тус бүрийн туслахуудыг хянахын тулд энэ төвийг ашиглана уу.
Зэвийн тусгай хэсгүүдийн хувьд [Rust SDK snippets](./developer-sdk-rust.md) руу очно уу.

## Хэлний туслахууд

- **Python** — `sorafs_multi_fetch_local` (орон нутгийн оркестрын утааны туршилт) болон
  `sorafs_gateway_fetch` (Gateway E2E дасгалууд) одоо нэмэлт сонголтыг хүлээн зөвшөөрч байна.
  `telemetry_region` дээр нэмэх нь `transport_policy` дарсан
  (`"soranet-first"`, `"soranet-strict"`, эсвэл `"direct-only"`), CLI-г толин тусгал
  эргүүлэх товчлуурууд. Орон нутгийн QUIC прокси эргэх үед,
  `sorafs_gateway_fetch` доорх хөтчийн манифестийг буцаана
  `local_proxy_manifest` тул тест нь хөтчийн адаптеруудад итгэлцлийн багцыг өгөх боломжтой.
- **JavaScript** — `sorafsMultiFetchLocal` нь Python туслагчийг харуулж, буцаж ирдэг
  ачааны байт болон хүлээн авсан хураангуй, харин `sorafsGatewayFetch` дасгал
  Torii гарцууд, локал прокси манифестуудыг дамжуулж, ижил зүйлийг ил гаргадаг
  телеметрийн/тээврийн хэрэглүүрийг CLI гэж дарна.
- **Rust** — үйлчилгээнүүд нь хуваарийг шууд дамжуулан оруулах боломжтой
  `sorafs_car::multi_fetch`; [Rust SDK хэсгүүдийг](./developer-sdk-rust.md) үзнэ үү
  proof-stream туслахууд болон найруулагчийн интеграцийн лавлагаа.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` нь Torii HTTP-г дахин ашигладаг
  гүйцэтгэгч, хүндэт `GatewayFetchOptions`. Үүнтэй хослуул
  `ClientConfig.Builder#setSorafsGatewayUri` болон PQ байршуулах зөвлөмж
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) байршуулалт нь хадгалагдах ёстой
  Зөвхөн PQ замууд.

## Онооны самбар болон бодлогын товчлуурууд

Python (`sorafs_multi_fetch_local`) болон JavaScript хоёулаа
(`sorafsMultiFetchLocal`) туслахууд телеметрийг мэддэг хуваарилагчийн онооны самбарыг харуулж байна
CLI ашигладаг:

- Үйлдвэрлэлийн хоёртын файлууд нь онооны самбарыг анхдагчаар идэвхжүүлдэг; `use_scoreboard=True` тохируулна
  (эсвэл `telemetry` оруулгуудыг өгнө үү) туслагчийг гаргахын тулд бэхэлгээг дахин тоглуулах үед
  зар сурталчилгааны мета өгөгдөл болон сүүлийн үеийн телеметрийн агшин зуурын зургуудаас жигнэсэн үйлчилгээ үзүүлэгчийн захиалга.
- Тооцоолсон жинг хэсгүүдийн хажууд хүлээн авахын тулд `return_scoreboard=True`-г тохируулна уу.
  хүлээн авсан тул CI бүртгэл нь оношилгоог бичих боломжтой.
- `deny_providers` эсвэл `boost_providers` массивуудыг ашиглан үе тэнгийнхнээсээ татгалзах эсвэл нэмэх
  Төлөвлөгч үйлчилгээ үзүүлэгчдийг сонгох үед `priority_delta`.
- Зэрэглэлийг бууруулаагүй л бол `"soranet-first"`-ийн өгөгдмөл байрлалыг хадгалах; нийлүүлэлт
  `"direct-only"` зөвхөн дагаж мөрдөх бүс нь релеээс зайлсхийх ёстой эсвэл хэзээ
  SNNet-5a нөөцийг давтаж, `"soranet-strict"`-г зөвхөн PQ-д зориулж нөөцлөх
  засаглалын зөвшөөрөлтэй нисгэгчид.
- Gateway туслахууд нь мөн `scoreboardOutPath` болон `scoreboardNowUnixSecs`-г ил гаргадаг.
  Тооцоолсон онооны самбарыг үргэлжлүүлэхийн тулд `scoreboardOutPath`-г тохируулна уу (CLI-г тусгана).
  `--scoreboard-out` туг) тул `cargo xtask sorafs-adoption-check` баталгаажуулах боломжтой
  SDK олдворууд ба бэхэлгээнд тогтвортой байх шаардлагатай үед `scoreboardNowUnixSecs` ашиглана уу.
  Хуулбарлах боломжтой мета өгөгдлийн `assume_now` утга. JavaScript туслагч дээр та
  нэмэлт `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` тохируулж болно;
  шошгыг орхигдуулсан үед `region:<telemetryRegion>` (буцаж байна
  `sdk:js`). Python туслагч автоматаар `telemetry_source="sdk:python"` ялгаруулдаг
  онооны самбар хэвээр байх ба далд мета өгөгдлийг идэвхгүй байлгах бүрт.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```