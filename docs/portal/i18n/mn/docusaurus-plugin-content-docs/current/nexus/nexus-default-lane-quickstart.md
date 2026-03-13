---
id: nexus-default-lane-quickstart
lang: mn
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
Энэ хуудас нь `docs/source/quickstart/default_lane.md`-ийг толилуулж байна. Хоёр хуулбарыг хоёуланг нь хадгал
нутагшуулах шүүрэлтийг портал дээр буулгах хүртэл зэрэгцүүлнэ.
:::

# Өгөгдмөл эгнээний хурдан эхлүүлэх (NX-5)

> **Замын зураглалын нөхцөл:** NX-5 — нийтийн эгнээний анхдагч нэгдэл. Одоо ажиллах хугацаа
> `nexus.routing_policy.default_lane` буцаалтыг ил гаргаснаар Torii REST/gRPC
> төгсгөлийн цэгүүд болон SDK бүр замын хөдөлгөөнд хамаарах үед `lane_id`-г аюулгүй орхиж болно.
> каноник нийтийн эгнээнд. Энэхүү гарын авлага нь операторуудыг тохиргоо хийх замаар дамждаг
> каталог, `/status` дахь нөөцийг баталгаажуулж, үйлчлүүлэгчийг ажиллуулах
> зан үйлийн төгсгөл хүртэл.

## Урьдчилсан нөхцөл

- `irohad`-ийн Sora/Nexus бүтээц (`irohad --sora --config ...`-тэй ажилладаг).
- Та `nexus.*` хэсгүүдийг засах боломжтой тохиргооны агуулах руу нэвтрэх.
- `iroha_cli` нь зорилтот кластертай ярихаар тохируулагдсан.
- `curl`/`jq` (эсвэл түүнтэй адилтгах) Torii `/status` ачааллыг шалгах.

## 1. Lane болон dataspace каталогийг тайлбарла

Сүлжээнд байх ёстой эгнээ болон мэдээллийн орон зайг зарла. Хэсэг
доор (`defaults/nexus/config.toml`-аас тайрсан) гурван нийтийн эгнээг бүртгэдэг
нэмээд тохирох өгөгдлийн орон зайн нэрс:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

`index` бүр өвөрмөц бөгөөд залгаа байх ёстой. Dataspace ID нь 64 битийн утгууд юм;
Дээрх жишээнүүд нь тодорхой болгох үүднээс эгнээний индексүүдтэй ижил тоон утгуудыг ашигладаг.

## 2. Чиглүүлэлтийн өгөгдмөл болон нэмэлт тохируулгыг тохируулна уу

`nexus.routing_policy` хэсэг нь буцах эгнээг хянаж, танд олгодог
тодорхой зааварчилгаа эсвэл дансны угтварын чиглүүрийг дарж бичих. Хэрэв дүрэм байхгүй бол
таарч байвал төлөвлөгч нь гүйлгээг тохируулсан `default_lane` руу чиглүүлдэг.
болон `default_dataspace`. Чиглүүлэгчийн логик дотор амьдардаг
`crates/iroha_core/src/queue/router.rs` ба бодлогыг ил тод байдлаар хэрэгжүүлдэг
Torii REST/gRPC гадаргуу.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Дараа нь шинэ эгнээ нэмэхдээ эхлээд каталогийг шинэчилж, дараа нь чиглүүлээ сунгана уу
дүрэм. Буцах эгнээ нь барьж буй нийтийн эгнээ рүү чиглэж байх ёстой

## 3. Бодлогын дагуу зангилаа ачаална уу

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Зангилаа нь эхлүүлэх явцад үүссэн чиглүүлэлтийн бодлогыг бүртгэдэг. Аливаа баталгаажуулалтын алдаа
(дугасан индекс, давхардсан нэр, хүчингүй өгөгдлийн орон зайн id) өмнө гарч ирнэ.
хов жив эхэлдэг.

## 4. Эгнээний засаглалын төлөвийг баталгаажуулах

Зангилаа онлайн болсны дараа өгөгдмөл эгнээ байгаа эсэхийг шалгахын тулд CLI туслахыг ашиглана уу
битүүмжилсэн (манифест ачаалагдсан) болон хөдөлгөөнд бэлэн байна. Хураангуй харагдац нь нэг мөр хэвлэдэг
нэг эгнээнд:

```bash
iroha_cli app nexus lane-report --summary
```

Жишээ гаралт:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Хэрэв өгөгдмөл эгнээ нь `sealed`-г харуулж байвал эгнээний удирдлагын дэвтрийг дагаж мөрдөөрэй.
гадаад урсгалыг зөвшөөрөх. `--fail-on-sealed` туг нь CI-д тохиромжтой.

## 5. Torii төлөвийн ачааллыг шалгана уу

`/status` хариулт нь чиглүүлэлтийн бодлого болон нэг эгнээний хуваарийг хоёуланг нь илчилдэг.
агшин зуурын зураг. Тохируулсан өгөгдмөлүүдийг баталгаажуулахын тулд `curl`/`jq`-г ашиглана уу.
Буцах зурвас нь телеметрийг үүсгэж байна:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Жишээ гаралт:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

`0` эгнээний шууд хуваарийн тоолуурыг шалгахын тулд:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Энэ нь TEU агшин зуурын зураг, мета өгөгдөл болон манифестийн тугуудыг зэрэгцүүлж байгааг баталж байна.
тохиргооны хамт. Grafana хавтангууд нь ижил ачаалалтай байдаг
lane-ingest хяналтын самбар.

## 6. Үйлчлүүлэгчийн өгөгдмөл тохиргоог хийх

- **Rust/CLI.** `iroha_cli` болон Rust клиент хайрцаг нь `lane_id` талбарыг орхигдуулсан.
  та `--lane-id` / `LaneSelector` тэнцээгүй үед. Тиймээс дараалал чиглүүлэгч
  `default_lane` руу буцна. Ил тод `--lane-id`/`--dataspace-id` туг ашиглах
  зөвхөн өгөгдмөл бус эгнээг чиглүүлэх үед.
- **JS/Swift/Android.** Хамгийн сүүлийн үеийн SDK хувилбарууд нь `laneId`/`lane_id`-г нэмэлт гэж үздэг.
  мөн `/status`-ийн сурталчилсан утга руу буцна. Чиглүүлэлтийн бодлогыг дотор нь байлга
  Үзүүлэлт болон үйлдвэрлэлийн хооронд синхрончлох тул гар утасны програмуудад яаралтай тусламж хэрэггүй
  дахин тохируулга.
- ** Дамжуулах хоолой/SSE тестүүд.** Гүйлгээний үйл явдлын шүүлтүүрүүд хүлээн авдаг
  `tx_lane_id == <u32>` предикатууд (`docs/source/pipeline.md`-г үзнэ үү). -д бүртгүүлнэ үү
  `/v2/pipeline/events/transactions` гэсэн шүүлтүүрийг илгээсэн гэдгийг батлахын тулд
  тодорхой эгнээгүйгээр буцах эгнээний id-ийн доор ирдэг.

## 7. Ажиглалт ба засаглалын дэгээ

- `/status` мөн `nexus_lane_governance_sealed_total` болон
  `nexus_lane_governance_sealed_aliases` тиймээс Alertmanager хэзээ ч анхааруулж болно
  эгнээ нь манифестээ алддаг. Эдгээр сэрэмжлүүлгийг devnet-д хүртэл идэвхжүүлсэн хэвээр байгаарай.
- Төлөөлөгчийн телеметрийн зураг болон эгнээний удирдлагын самбар
  (`dashboards/grafana/nexus_lanes.json`) -аас alias/slug талбаруудыг хүлээж байна
  каталог. Хэрэв та өөр нэрийн нэрийг өөрчилбөл харгалзах Кура лавлахуудыг дахин шошго
  аудиторууд детерминистик замыг (NX-1 дагуу хянадаг) хадгалдаг.
-УИХ-аас өгөгдмөл замуудыг батлахдаа буцаах төлөвлөгөөг тусгасан байх ёстой. Бичлэг
  илэрхий хэш ба засаглалын нотлох баримт нь энэ хурдан эхлэлийн хажууд таны
  оператор runbook тул ирээдүйн эргэлтүүд шаардлагатай төлөвийг тааварлахгүй.

Эдгээр шалгалтыг давсны дараа та `nexus.routing_policy.default_lane`-г шалгаж болно
сүлжээн дэх код замууд.