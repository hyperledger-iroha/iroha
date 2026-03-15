---
id: pin-registry-ops
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

## Тойм

Энэхүү runbook нь SoraFS пин бүртгэл болон түүний хуулбарлах үйлчилгээний түвшний гэрээг (SLAs) хэрхэн хянах, хянах талаар баримтжуулсан болно. Хэмжигдэхүүнүүд нь `iroha_torii`-ээс гаралтай бөгөөд `torii_sorafs_*` нэрийн зай дор Prometheus-ээр экспортлогдож байна. Torii нь бүртгэлийн төлөвийг арын дэвсгэр дээр 30 секундын интервалаар түүвэрлэдэг тул `/v2/sorafs/pin/*` төгсгөлийн цэгүүдэд ямар ч оператор санал өгөхгүй байсан ч хяналтын самбарууд хэвээр үлдэнэ. Доорх хэсгүүдэд шууд буулгах Grafana загварт ашиглахад бэлэн байгаа хяналтын самбарыг (`docs/source/grafana_sorafs_pin_registry.json`) импортлох.

## Метрийн лавлагаа

| Метрик | Шошго | Тодорхойлолт |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Амьдралын мөчлөгийн төлөвөөр гинжин манифест тооллого. |
| `torii_sorafs_registry_aliases_total` | — | Бүртгэлд бүртгэгдсэн идэвхтэй манифестын нэрсийн тоо. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Хуулбарлах захиалгын нөөцийг статусаар нь ангилсан. |
| `torii_sorafs_replication_backlog_total` | — | `pending` захиалгын толин тусгалтай тохиромжтой хэмжигч. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA нягтлан бодох бүртгэл: `met` нь эцсийн хугацаанд дууссан захиалгыг тоолдог, `missed` нь хоцрогдсон гүйцэтгэл + дуусах хугацааг нэгтгэдэг, `pending` үлдэгдэл захиалгыг тусгадаг. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Нэгдсэн дуусгах хоцрогдол (гаргах ба дуусгах хоорондох хугацаа). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Хүлээгдэж буй захиалгын сул цонх (хугацаа хасах хугацаа). |

Бүх хэмжигчийг агшин зуурын зураг авах бүрт шинэчилдэг тул хяналтын самбарууд `1m` хэмжигдэхүүнээр эсвэл илүү хурдан түүвэрлэх ёстой.

## Grafana Хяналтын самбар

JSON хяналтын самбар нь операторын ажлын урсгалыг хамарсан долоон самбартай ирдэг. Хэрэв та захиалгат график бүтээхийг хүсч байвал лавлагаа авахын тулд доорх асуултуудыг жагсаасан болно.

1. **Манифестийн амьдралын мөчлөг** – `torii_sorafs_registry_manifests_total` (`status`-ээр бүлэглэсэн).
2. **Алиа каталогийн чиг хандлага** – `torii_sorafs_registry_aliases_total`.
3. **Захиалгын дарааллыг статусаар нь гаргах** – `torii_sorafs_registry_orders_total` (`status`-р бүлэглэсэн).
4. **Хоцрогдсон бүртгэл ба хугацаа нь дууссан захиалга** – гадаргуугийн ханасан байдалд `torii_sorafs_replication_backlog_total` болон `torii_sorafs_registry_orders_total{status="expired"}`-ийг нэгтгэдэг.
5. **SLA амжилтын харьцаа** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Хоцролт ба эцсийн хугацаа сул** – давхарласан `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` болон `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Та туйлын сул шал хэрэгтэй үед `min_over_time` харагдац нэмэхийн тулд Grafana хувиргалтыг ашиглана уу, жишээлбэл:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Алдагдсан захиалга (1 цагийн хувь)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Сэрэмжлүүлгийн босго

- **SLA амжилт < 0.95 15 минут**
  - Босго: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Үйлдэл: Хуудас SRE; хуулбарлах хоцрогдол гурвыг эхлүүлэх.
- **10-аас дээш хүлээгдэж буй хоцрогдол**
  - Босго: `torii_sorafs_replication_backlog_total > 10` 10 минутын турш тогтвортой байна
  - Үйлдэл: Үйлчилгээ үзүүлэгчийн бэлэн байдал болон Torii хүчин чадал төлөвлөгчийг шалгана уу.
- **Хугацаа нь дууссан захиалга > 0**
  - Босго: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Үйлдэл: Үйлчилгээ үзүүлэгчийн гацааг баталгаажуулахын тулд засаглалын манифестуудыг шалгана уу.
- **Дуусгах p95 > эцсийн хугацаа суларсан дундаж**
  - Босго: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Үйлдэл: Үйлчилгээ үзүүлэгч нар эцсийн хугацаанаас өмнө үйл ажиллагаа явуулж байгаа эсэхийг шалгах; дахин томилолт олгох асуудлыг авч үзэх.

### Жишээ Prometheus дүрмүүд

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Гурвалсан ажлын урсгал

1. **Шалтгааныг тодорхойлох**
   - Хоцрогдол бага хэвээр байхад SLA огцом өсөлтийг алдвал үйлчилгээ үзүүлэгчийн гүйцэтгэлд анхаарлаа хандуулаарай (PoR алдаа, хожуу дуусгах).
   - Тогтвортой хоцрогдолтойгоор хоцрогдол нэмэгдвэл зөвлөлийн зөвшөөрлийг хүлээж буй манифестуудыг баталгаажуулахын тулд элсэлтийг шалгана уу (`/v2/sorafs/pin/*`).
2. **Үйлчилгээ үзүүлэгчийн статусыг баталгаажуулах**
   - `iroha app sorafs providers list`-г ажиллуулж, сурталчилсан чадварууд хуулбарлах шаардлагад нийцэж байгаа эсэхийг шалгаарай.
   - Заасан GiB болон PoR амжилтыг баталгаажуулахын тулд `torii_sorafs_capacity_*` хэмжигчийг шалгана уу.
3. **Хуулбарыг дахин хуваарилах**
   - Хоцрогдол (`stat="avg"`) 5 эрин үеэс доош унах үед `sorafs_manifest_stub capacity replication-order`-ээр дамжуулан шинэ захиалга гаргана (манифест/МАШИНЫ савлагаанд `iroha app sorafs toolkit pack` ашигладаг).
   - Хэрэв хуурамч нэрэнд идэвхтэй манифест холбоос байхгүй бол засаглалд мэдэгдэх (`torii_sorafs_registry_aliases_total` гэнэтийн уналт).
4. **Баримт бичгийн үр дүн**
   - Ослын тэмдэглэлийг SoraFS үйлдлийн бүртгэлд цагийн тэмдэг болон нөлөөлөлд өртсөн манифест дижестийн хамт тэмдэглэнэ үү.
   - Шинэ бүтэлгүйтлийн горим эсвэл хяналтын самбар нэвтрүүлсэн тохиолдолд энэ runbook-ийг шинэчилнэ үү.

## Дамжуулах төлөвлөгөө

Үйлдвэрлэлд нэрийн кэш бодлогыг идэвхжүүлэх эсвэл чангатгахдаа энэ үе шаттай процедурыг дагана уу:

1. **Тохиргоог бэлтгэх**
   - `torii.sorafs_alias_cache`-г `iroha_config` (хэрэглэгч → бодит) дээр тохиролцсон TTL болон нэмэлт цонхоор шинэчилнэ үү: `positive_ttl`, `refresh_window`, `hard_expiry`, I10802, I180802 `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Өгөгдмөл нь `docs/source/sorafs_alias_policy.md` дээрх бодлоготой таарч байна.
   - SDK-н хувьд ижил утгуудыг тохиргооны давхаргуудаараа (Rust / NAPI / Python холболтууд дахь `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`) хуваарилснаар үйлчлүүлэгчийн хэрэгжилт нь гарцтай таарч байна.
2. **Үзүүлэлтэд хуурай гүйлт**
   - Тохиргооны өөрчлөлтийг үйлдвэрлэлийн топологийг тусгасан шатлалын кластерт байрлуулна.
   - `cargo xtask sorafs-pin-fixtures`-г ажиллуулж, каноник нэрийн төхөөрөмжүүдийн кодыг тайлж, хоёр тийш эргэхийг баталгаажуулах; аливаа үл нийцэл нь эхлээд шийдвэрлэх ёстой дээд урсгалын илэрхий шилжилтийг илэрхийлдэг.
   - `/v2/sorafs/pin/{digest}` болон `/v2/sorafs/aliases` төгсгөлийн цэгүүдийг шинэ, шинэчлэх цонх, хугацаа нь дууссан, хугацаа нь дууссан тохиолдлуудыг хамарсан синтетик баталгаатай дасгал хийнэ. HTTP төлөвийн код, толгой хэсэг (`Sora-Proof-Status`, `Retry-After`, `Warning`) болон JSON үндсэн талбаруудыг энэ runbook-ийн эсрэг баталгаажуулна уу.
3. **Үйлдвэрлэлд идэвхжүүлэх**
   - Стандарт өөрчлөлтийн цонхоор шинэ тохиргоог хийнэ үү. Үүнийг эхлээд Torii-д хэрэглээрэй, дараа нь зангилаа бүртгэл дэх шинэ бодлогыг баталгаажуулсны дараа гарц/SDK үйлчилгээг дахин эхлүүлнэ үү.
   - `docs/source/grafana_sorafs_pin_registry.json`-г Grafana руу импортлох (эсвэл одоо байгаа хяналтын самбарыг шинэчлэх) болон NOC-ийн ажлын талбарт нэрийн кэш шинэчлэх самбарыг хавчуулна.
4. **Байршуулсаны дараах баталгаажуулалт**
   - `torii_sorafs_alias_cache_refresh_total`, `torii_sorafs_alias_cache_age_seconds` 30 минутын турш хяналт тавина. `error`/`expired` муруй дахь огцом өсөлт нь бодлогын шинэчлэлтийн цонхтой хамааралтай байх ёстой; Гэнэтийн өсөлт нь операторууд үргэлжлүүлэхээсээ өмнө нэрийн баталгаа болон үйлчилгээ үзүүлэгчийн эрүүл мэндийг шалгах ёстой гэсэн үг юм.
   - Үйлчлүүлэгчийн лог нь ижил бодлогын шийдвэрийг харуулж байгааг баталгаажуулах (Баталгаа хуучирсан эсвэл хугацаа нь дууссан үед SDK нь алдаа гаргадаг). Үйлчлүүлэгчийн анхааруулга байхгүй байгаа нь тохиргоо буруу байгааг илтгэнэ.
5. **Буцах**
   - Хэрэв нэр өгөх хугацаа хоцорч, шинэчлэх цонх байнга алдагдаж байвал тохиргоонд `refresh_window` болон `positive_ttl`-г нэмэгдүүлэх замаар бодлогыг түр сулруулж, дахин байршуулна уу. `hard_expiry`-г бүрэн бүтэн байлгаснаар жинхэнэ хуучирсан нотолгоог үгүйсгэсэн хэвээр байх болно.
   - Хэрэв телеметр нь `error`-ийн өндөр тоог харуулсаар байвал өмнөх `iroha_config` агшин агшныг сэргээж өмнөх тохиргоо руугаа буцаад, дараа нь хуурамч нэр үүсгэх саатлыг хянахын тулд тохиолдлыг нээнэ үү.

## Холбогдох материалууд

- `docs/source/sorafs/pin_registry_plan.md` — хэрэгжүүлэх замын зураглал ба засаглалын нөхцөл.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — хадгалалтын ажилтны үйл ажиллагаа нь энэхүү бүртгэлийн дэвтрийг нөхөж өгдөг.