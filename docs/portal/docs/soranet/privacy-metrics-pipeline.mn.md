---
lang: mn
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9eafd2f3786dbe0ccfa7db2ec138d1d91e306e1691fbee801ba04a4165131655
source_last_modified: "2026-01-05T09:28:11.915061+00:00"
translation_last_reviewed: 2026-02-07
id: privacy-metrics-pipeline
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
:::

# SoraNet нууцлалын хэмжүүрийн шугам хоолой

SNNet-8 нь релений ажиллах хугацаанд нууцлалыг мэддэг телеметрийн гадаргууг танилцуулж байна. The
реле одоо гар барих болон хэлхээний үйл явдлуудыг минутын хэмжээтэй хувин болон
зөвхөн бүдүүн Prometheus тоолуурыг экспортолж, бие даасан хэлхээг хадгалдаг
салгах боломжгүй бөгөөд операторуудад үйлдэл хийх боломжтой харагдац өгдөг.

## Агрегаторын тойм

- Ажиллах цагийн хэрэгжилт нь `tools/soranet-relay/src/privacy.rs` байдлаар амьдардаг
  `PrivacyAggregator`.
- Хувинг ханын цагийн минутаар (`bucket_secs`, өгөгдмөл 60 секунд) тохируулдаг.
  хязгаарлагдмал цагирагт хадгалагдсан (`max_completed_buckets`, анхдагч 120). Цуглуулагч
  хувьцаанууд өөрсдийн хязгаарлагдмал нөөцийг хадгалдаг (`max_share_lag_buckets`, анхдагч 12)
  Тиймээс хуучирсан Prio цонхнууд гоожиж биш харин дарагдсан хувин шиг угаадаг
  санах ой эсвэл гацсан коллекторуудыг далдлах.
- `RelayConfig::privacy` `PrivacyConfig` руу шууд зураглаж, тааруулахыг ил гаргана.
  бариул (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`). Үйлдвэрлэлийн ажиллах хугацаа нь SNNet-8a үед анхдагч тохиргоог хадгалдаг
  аюулгүй нэгтгэх босгыг нэвтрүүлдэг.
- Ажиллах цагийн модулиуд нь бичсэн туслагчаар дамжуулан үйл явдлуудыг бүртгэдэг:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, `record_gar_category`.

## Relay Admin Endpoint

Операторууд релейн админ сонсогчоос түүхий ажиглалт хийх боломжтой
`GET /privacy/events`. Төгсгөлийн цэг нь шинэ мөрөөр тусгаарлагдсан JSON-г буцаана
(`application/x-ndjson`) нь толин тусгалтай `SoranetPrivacyEventV1` ачааллыг агуулсан
дотоод `PrivacyEventBuffer`-ээс. Буфер нь хамгийн сүүлийн үеийн үйл явдлуудыг хадгалдаг
`privacy.event_buffer_capacity` оролт руу (өгөгдмөл 4096) болон шавхагдсан байна
уншина уу, тиймээс хусагч нь цоорхой гарахгүйн тулд хангалттай олон удаа санал асуулга явуулах ёстой. Үйл явдлуудыг хамардаг
ижил гар барих, тохируулагч, баталгаажуулсан зурвасын өргөн, идэвхтэй хэлхээ, GAR дохио
Энэ нь Prometheus тоолуурыг тэжээж, доод цуглуулагчдад архивлах боломжийг олгодог.
нууцлалыг хамгаалсан талхны үйрмэгүүд эсвэл тэжээлийн аюулгүй нэгтгэх ажлын урсгалууд.

## Релений тохиргоо

Операторууд реле тохиргооны файл дахь нууцлалын телеметрийн хэмжигдэхүүнийг тохируулна
`privacy` хэсэг:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Талбарын өгөгдмөл нь SNNet-8 үзүүлэлттэй таарч байгаа бөгөөд ачаалах үед баталгаажуулна:

| Талбай | Тодорхойлолт | Өгөгдмөл |
|-------|-------------|---------|
| `bucket_secs` | Нэгтгэх цонх бүрийн өргөн (секунд). | `60` |
| `min_handshakes` | Нэг хувин тоолуур гаргахаас өмнө хамгийн бага хувь нэмэр оруулагчдын тоо. | `12` |
| `flush_delay_buckets` | Угаах оролдлого хийхээсээ өмнө хүлээнэ үү. | `1` |
| `force_flush_buckets` | Бид дарагдсан хувин ялгаруулахаас өмнөх хамгийн дээд нас. | `6` |
| `max_completed_buckets` | Хадгалагдсан хувин нөөц (хязгааргүй санах ойг сэргийлдэг). | `120` |
| `max_share_lag_buckets` | Дарахаас өмнө цуглуулагчийн хувьцааг хадгалах цонх. | `12` |
| `expected_shares` | Нэгтгэхийн өмнө цуглуулагчийн хувьцаа шаардлагатай. | `2` |
| `event_buffer_capacity` | Админ дамжуулалтад зориулсан NDJSON үйл явдлын хоцрогдол. | `4096` |

`force_flush_buckets` тохиргоог `flush_delay_buckets`-ээс доогуур болгож,
босго, эсвэл хадгалах хамгаалалтыг идэвхгүй болгосноор одоо зайлсхийхийн тулд баталгаажуулалт амжилтгүй боллоо
релейн телеметрийг алдагдуулахуйц байршуулалт.

`event_buffer_capacity` хязгаар нь мөн `/admin/privacy/events`-ийг хязгаарлаж,
хусуурууд тодорхойгүй хугацаагаар хоцорч болохгүй.

## Prio цуглуулагчийн хувьцаа

SNNet-8a нь нууц хуваалцсан Prio хувин ялгаруулдаг хос коллекторуудыг байрлуулдаг. The
Оркестр одоо `/privacy/events` NDJSON урсгалыг хоёуланд нь задлан шинжилж байна
`SoranetPrivacyEventV1` оруулгууд болон `SoranetPrivacyPrioShareV1` хувьцаанууд,
тэдгээрийг `SoranetSecureAggregator::ingest_prio_share` руу дамжуулах. Хувин ялгаруулдаг
нэг удаа `PrivacyBucketConfig::expected_shares` хувь нэмэр орж ирэх нь толин тусгал
реле зан үйл. Хувьцааг хувингийн зэрэгцүүлэлт болон гистограмын хэлбэрт тохируулан баталгаажуулсан
`SoranetPrivacyBucketMetricsV1`-д нэгтгэхээс өмнө. Хэрэв хосолсон бол
гар барих тоо `min_contributors`-ээс доош унавал хувиныг дараах байдлаар экспортлодог.
`suppressed`, реле дэх агрегаторын үйлдлийг тусгадаг. Дарагдсан
Windows одоо `suppression_reason` шошгыг ялгаруулдаг тул операторууд ялгах боломжтой
`insufficient_contributors`, `collector_suppressed`,
`collector_window_elapsed`, `forced_flush_window_elapsed` хувилбарууд
телеметрийн цоорхойг оношлох. `collector_window_elapsed` шалтгаан нь бас шатдаг
Prio-ийн хувьцаанууд `max_share_lag_buckets`-ийг давж, цуглуулагчдыг гацсан үед
санах ойд хуучирсан аккумляторыг үлдээхгүйгээр харагдана.

## Torii залгих төгсгөлийн цэгүүд

Torii нь одоо хоёр телеметрийн хаалгатай HTTP төгсгөлийн цэгийг ил гаргах тул реле болон коллекторууд
захиалгат тээвэрлэлт оруулахгүйгээр ажиглалтыг дамжуулах боломжтой:

- `POST /v1/soranet/privacy/event` зөвшөөрнө a
  `RecordSoranetPrivacyEventDto` ачаалал. Биеийг ороох a
  `SoranetPrivacyEventV1` болон нэмэлт `source` шошго. Torii баталгаажуулна
  идэвхтэй телеметрийн профайлын эсрэг хүсэлт гаргаж, үйл явдлыг бүртгэж, хариу өгнө
  HTTP `202 Accepted`-тай, Norito JSON дугтуйтай хамт
  тооцоолсон хувин цонх (`bucket_start_unix`, `bucket_duration_secs`) ба
  реле горим.
- `POST /v1/soranet/privacy/share` нь `RecordSoranetPrivacyShareDto`-г хүлээн авдаг
  ачаалал. Бие нь `SoranetPrivacyPrioShareV1` болон нэмэлт загвартай
  `forwarded_by` зөвлөмж нь операторууд коллекторын урсгалыг шалгах боломжтой. Амжилттай
  Ирүүлсэн материалууд нь HTTP `202 Accepted`-ийг Norito JSON дугтуйтай нэгтгэн харуулна.
  коллектор, шанаганы цонх, дарах зөвлөмж; баталгаажуулалтын алдааны зураглал
  детерминистик алдаатай ажиллахыг хадгалах телеметрийн `Conversion` хариу
  цуглуулагчид даяар. Оркестраторын үйл явдлын гогцоо одоо эдгээр хувьцааг өөр шигээ ялгаруулж байна
  санал асуулгын реле, Torii-ийн Prio аккумляторыг реле дээрх хувинтай синхрончлох.

Хоёр төгсгөлийн цэг хоёулаа телеметрийн профайлыг хүндэтгэдэг: тэд `503 үйлчилгээг ялгаруулдаг
Хэмжилтийг идэвхгүй болгосон үед боломжгүй`. Үйлчлүүлэгчид Norito хоёртын файлыг илгээж болно
(`application/x.norito`) эсвэл Norito JSON (`application/x.norito+json`) бие;
сервер нь стандарт Torii-ээр форматыг автоматаар хэлэлцдэг
олборлогч.

## Prometheus хэмжигдэхүүн

Экспортолсон хувин бүрт `mode` (`entry`, `middle`, `exit`) болон
`bucket_start` шошго. Дараах хэмжигдэхүүнүүдийн гэр бүлүүд ялгардаг:

| Метрик | Тодорхойлолт |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` бүхий гар барих ангилал зүй. |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` бүхий тохируулагч тоолуур. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Тохиромжтой гар барихад нөлөөлсөн нийт хөргөлтийн хугацаа. |
| `soranet_privacy_verified_bytes_total` | Сохор хэмжилтийн нотолгооноос баталгаажуулсан зурвасын өргөн. |
| `soranet_privacy_active_circuits_{avg,max}` | Нэг хувин дахь дундаж ба оргил идэвхтэй хэлхээ. |
| `soranet_privacy_rtt_millis{percentile}` | RTT хувийн тооцоолол (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Засаглалын үйл ажиллагааны тайлангийн тоолуурыг категорийн тоймоор оруулсан. |
| `soranet_privacy_bucket_suppressed` | Оролцогчийн босгыг хангаагүй тул хувин суутгалсан. |
| `soranet_privacy_pending_collectors{mode}` | Реле горимоор бүлэглэсэн, хүлээгдэж буй хослолыг цуглуулагч хуваалцдаг. |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` бүхий хувин тоолуурыг дарсан тул хяналтын самбар нь нууцлалын цоорхойг тодорхойлох боломжтой. |
| `soranet_privacy_snapshot_suppression_ratio` | Сүүлчийн ус зайлуулах шугамын дарагдсан/усгүй болсон харьцаа (0–1) нь анхааруулах төсөвт тустай. |
| `soranet_privacy_last_poll_unixtime` | Хамгийн сүүлийн амжилттай санал асуулгын UNIX цагийн тэмдэг (цуглуулагчийн идэвхгүй байдлын дохиоллыг жолооддог). |
| `soranet_privacy_collector_enabled` | Нууцлалын цуглуулагч идэвхгүй болсон эсвэл эхлэхгүй байх үед `0` руу шилждэг хэмжигч (коллекторыг идэвхгүй болгосон дохиог ажиллуулдаг). |
| `soranet_privacy_poll_errors_total{provider}` | Санал асуулгын алдааг реле нэрээр бүлэглэсэн (код тайлах алдаа, HTTP алдаа эсвэл гэнэтийн төлөвийн кодын өсөлт). |

Ажиглалтгүй хувин чимээгүй байж, хяналтын самбарыг эмх цэгцтэй байлгадаг
тэг дүүргэсэн цонх үйлдвэрлэх.

## Үйл ажиллагааны удирдамж

1. **Хяналтын самбар** – дээрх хэмжүүрүүдийг `mode` болон `window_start`-р бүлэглэсэн графикаар зур.
   Коллектор эсвэл релений асуудлыг шийдэхийн тулд дутуу цонхнуудыг тодруулна уу. Ашиглах
   Хувь нэмэр оруулагчийг ялгахын тулд `soranet_privacy_suppression_total{reason}`
   цоорхойг илрүүлэх үед коллекторын удирдлагатай дарангуйллаас үүсэх дутагдал. Grafana
   Өмч одоо тэдгээрээс тэжээгддэг тусгай зориулалтын **“Дарангуйлах шалтгаанууд (5м)”** самбарыг илгээдэг.
   тоолуур дээр нэмээд тооцдог **“Дарагдсан хувин %”** статистик
   Сонголт бүрт `sum(soranet_privacy_bucket_suppressed) / count(...)` тийм
   операторууд төсвийн зөрчлийг нэг дор анзаарах боломжтой. **Цуглуулагчийн хувьцаа
   Хоцрогдол** цуврал (`soranet_privacy_pending_collectors`) болон **Ачмын хувилбар
   Дарах харьцаа** стат нь гацсан цуглуулагчид болон төсвийн зарцуулалтыг онцолж өгдөг
   автоматжуулсан гүйлтүүд.
2. **Сэрэмжлүүлэг** – нууцлалыг хамгаалсан тоолуураас дохиолол өгөх: PoW-ийн огцом өсөлт,
   хөргөлтийн давтамж, RTT шилжилт, багтаамжаас татгалздаг. Учир нь тоолуур байдаг
   хувин бүрийн дотор монотон, шууд ханш дээр суурилсан дүрэм сайн ажилладаг.
3. **Ойл явдлын хариу** – эхлээд нэгтгэсэн өгөгдөлд тулгуурлана. Илүү гүнзгий дибаг хийх үед
   шаардлагатай бол хувин агшин зуурын зургийг дахин тоглуулах эсвэл сохорсон эсэхийг шалгахын тулд реле хүсэх
   түүхий замын хөдөлгөөний бүртгэлийг хураахын оронд хэмжилтийн нотолгоо.
4. **Хадгалах** – хэтрүүлэхгүйн тулд хангалттай олон удаа хусах
   `max_completed_buckets`. Экспортлогчид Prometheus гарцыг
   каноник эх сурвалж ба орон нутгийн хувинуудыг нэг удаа дамжуулсан.

## Дарах аналитик ба автомат гүйлтSNNet-8-ыг хүлээн авах нь автоматжуулсан цуглуулагчид үлддэг гэдгийг харуулахаас шалтгаална
эрүүл бөгөөд энэ дарангуйлал бодлогын хүрээнд (≤10% хувин тутамд) хэвээр байна
дурын 30 минутын цонхоор дамжуулна). Одоо тэр хаалгыг хангахын тулд багаж хэрэгсэл хэрэгтэй байв
модтой хөлөг онгоцууд; операторууд үүнийг долоо хоног бүрийн зан үйлдээ оруулах ёстой. Шинэ
Grafana дарах самбарууд нь доорх PromQL хэсгүүдийг тусгаж, дуудлагаар өгөх
багууд гарын авлагын асуулгад буцаж орохоосоо өмнө шууд харагдацтай байдаг.

### Дарангуйллыг хянах PromQL жор

Операторууд дараах PromQL туслахуудыг гартаа байлгах ёстой; хоёуланг нь иш татсан
хуваалцсан Grafana хяналтын самбарт (`dashboards/grafana/soranet_privacy_metrics.json`)
болон Alertmanager дүрэм:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Харьцааны гаралтыг ашиглан **“Дарагдсан хувин %”** статистик доор байгаа эсэхийг баталгаажуулна уу
бодлогын төсөв; хурдан хариу өгөхийн тулд баяжуулалтын мэдрэгчийг Alertmanager руу холбоно уу
Оролцогчийн тоо гэнэт буурах үед.

### Офлайн хувин тайлан CLI

Ажлын талбар нь нэг удаагийн NDJSON-д зориулж `cargo xtask soranet-privacy-report`-г харуулж байна.
барьж авдаг. Үүнийг нэг буюу хэд хэдэн реле админ экспорт руу чиглүүлнэ:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Туслагч нь `SoranetSecureAggregator`-ээр зураг авалтыг дамжуулж, a
дарах хураангуйг stdout руу оруулах ба сонголтоор бүтэцлэгдсэн JSON тайланг бичнэ
`--json-out <path|->`-ээр дамжуулан. Энэ нь амьд цуглуулагчтай ижил товчлууруудыг хүндэтгэдэг
(`--bucket-secs`, `--min-contributors`, `--expected-shares` гэх мэт), түрээслэх
Операторууд туршилт хийхдээ өөр өөр босго дор түүхэн зураг авалтыг дахин тоглуулдаг
асуудал. JSON-г Grafana дэлгэцийн агшинд хавсаргаснаар SNNet-8
дарангуйлах аналитик хаалга аудит боломжтой хэвээр байна.

### Анхны автомат гүйлтийн хяналтын хуудас

Засаглал нь анхны автоматжуулалт нь шаардлага хангасан гэдгийг нотлохыг шаарддаг
дарангуйлах төсөв. Туслагч одоо `--max-suppression-ratio <0-1>`-г хүлээн зөвшөөрч байна
Дарагдсан хувин зөвшөөрөгдсөн хэмжээнээс хэтэрсэн тохиолдолд CI эсвэл операторууд хурдан бүтэлгүйтдэг
цонх (өгөгдмөл 10%) эсвэл хувин байхгүй үед. Санал болгож буй урсгал:

1. NDJSON-г релей админ төгсгөлийн цэгээс болон найруулагчийн цэгээс экспортлох
   `/v1/soranet/privacy/event|share` руу урсдаг
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Туслагчийг бодлогын төсвөөр ажиллуулна уу:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Энэ тушаал нь ажиглагдсан харьцааг хэвлэж, төсөв бэлэн болсон үед тэгээс өөр гарна
   хувин бэлэн болоогүй үед **эсвэл** хэтэрсэн нь телеметрийн хувьд бэлэн болоогүй байгааг харуулж байна
   гүйлтэнд зориулж үйлдвэрлэгдээгүй байна. Шууд хэмжүүрүүдийг харуулах ёстой
   `soranet_privacy_pending_collectors` тэг рүү урсах ба
   `soranet_privacy_snapshot_suppression_ratio` ижил төсөвт үлдэж байна
   гүйлт явагдаж байх хооронд.
3. Өмнө нь SNNet-8 нотлох баримтын багцаар JSON гаралт болон CLI бүртгэлийг архивлах
   Шүүгчид яг олдворуудыг дахин тоглуулахын тулд тээвэрлэлтийн өгөгдмөлийг эргүүлэх.

## Дараагийн алхамууд (SNNet-8a)

- Давхар Prio цуглуулагчийг нэгтгэж, тэдний хувьцааны залгилыг сүлжээнд холбоно
  ажиллах хугацаа тул реле болон коллекторууд тогтмол `SoranetPrivacyBucketMetricsV1` ялгаруулдаг
  ачаалал. *(Дууссан - `ingest_privacy_payload`-г үзнэ үү
  `crates/sorafs_orchestrator/src/lib.rs` ба дагалдах туршилтууд.)*
- Хуваалцсан Prometheus JSON хяналтын самбар болон анхааруулах дүрмийг нийтлэх
  дарах цоорхой, коллекторын эрүүл мэнд, нэрээ нууцлах . *(Дууссан - үзнэ үү
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, баталгаажуулах хэрэгсэл.)*
--д тайлбарласан дифференциал-нууцлалын тохируулгын олдворуудыг гаргах
  `privacy_metrics_dp.md`, хуулбарлах дэвтэр, засаглал зэрэг
  боловсруулдаг. *(Дууссан — дэвтэр + үүсгэсэн олдворууд
  `scripts/telemetry/run_privacy_dp.py`; CI боодол
  `scripts/telemetry/run_privacy_dp_notebook.sh` нь тэмдэглэлийн дэвтэрийг дамжуулан гүйцэтгэдэг
  `.github/workflows/release-pipeline.yml` ажлын урсгал; засаглалын тоймыг оруулсан
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

Одоогийн хувилбар нь SNNet-8 суурийг хүргэж байна: детерминистик,
Одоо байгаа Prometheus хусагч руу шууд ордог нууцлалын аюулгүй телеметр
болон хяналтын самбарууд. Дифференциал нууцлалын шалгалт тохируулгын олдворууд байгаа, the
дамжуулах хоолойн ажлын урсгал нь дэвтэрийн гаралтыг шинэ, үлдсэнийг нь хадгалдаг
ажил нь эхний автоматжуулсан гүйлтийг хянах, түүнчлэн дарангуйллыг өргөтгөхөд чиглэгддэг
дохиоллын аналитик.