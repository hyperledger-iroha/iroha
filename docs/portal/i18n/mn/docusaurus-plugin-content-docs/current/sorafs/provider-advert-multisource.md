---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Олон эх сурвалжийн үйлчилгээ үзүүлэгчийн сурталчилгаа ба хуваарь

Энэ хуудас нь каноник үзүүлэлтийг нэрлэсэн
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Norito схем болон өөрчлөлтийн бүртгэлд тухайн баримт бичгийг үгчлэн ашиглах; портал хуулбар
Операторын зааварчилгаа, SDK тэмдэглэл, телеметрийн лавлагааг бусадтай нь ойр байлгадаг
SoraFS runbook-ийн .

## Norito схемийн нэмэлтүүд

### Хүрээний чадвар (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – хүсэлт бүрт хамгийн том залгаа зай (байт), `≥ 1`.
- `min_granularity` – шийдэл хайх, `1 ≤ value ≤ max_chunk_span`.
- `supports_sparse_offsets` – нэг хүсэлтээр залгаагүй офсет хийхийг зөвшөөрдөг.
- `requires_alignment` – үнэн үед офсетууд `min_granularity`-тай таарч байх ёстой.
- `supports_merkle_proof` – PoR гэрчийн дэмжлэгийг харуулж байна.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` каноник кодчилолыг хэрэгжүүлэх
тиймээс хов живийн ачаалал тодорхойлогддог хэвээр байна.

### `StreamBudgetV1`
- Талбарууд: `max_in_flight`, `max_bytes_per_sec`, нэмэлт `burst_bytes`.
- Баталгаажуулах дүрэм (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, байгаа бол `> 0` болон `≤ max_bytes_per_sec` байх ёстой.

### `TransportHintV1`
- Талбарууд: `protocol: TransportProtocol`, `priority: u8` (0–15 цонхыг хэрэгжүүлсэн
  `TransportHintV1::validate`).
- Мэдэгдэж буй протоколууд: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Үйлчилгээ үзүүлэгч бүрт давхардсан протоколын оруулгуудаас татгалзсан.

### `ProviderAdvertBodyV1` нэмэлтүүд
- Нэмэлт `stream_budget: Option<StreamBudgetV1>`.
- Нэмэлт `transport_hints: Option<Vec<TransportHintV1>>`.
- Хоёр талбар одоо `ProviderAdmissionProposalV1`, засаглалаар дамждаг
  дугтуй, CLI бэхэлгээ, телеметрийн JSON.

## Баталгаажуулалт ба засаглалын үүрэг

`ProviderAdvertBodyV1::validate` болон `ProviderAdmissionProposalV1::validate`
алдаатай мета өгөгдлийг татгалзах:

- Хүрээний чадавхи нь тайлж, цар хүрээ/мянгийн хязгаарыг хангах ёстой.
- Дамжуулалтын төсөв / тээврийн зөвлөмжүүд нь тохирохыг шаарддаг
  `CapabilityType::ChunkRangeFetch` TLV болон хоосон бус зөвлөмжийн жагсаалт.
- Давхардсан тээврийн протокол, хүчингүй тэргүүлэх чиглэлүүд нь баталгаажуулалтыг нэмэгдүүлдэг
  зар сурталчилгааг хов жив ярихаас өмнөх алдаа.
- Элсэлтийн дугтуйнууд нь хүрээний мета өгөгдлийн санал/сурталчилгааг харьцуулна
  `compare_core_fields` тул таарахгүй хов живийн ачааллыг эрт үгүйсгэдэг.

Регрессийн хамрах хүрээ амьдардаг
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Багаж хэрэгсэл ба бэхэлгээ

- Үйлчилгээ үзүүлэгчийн сурталчилгааны ачаалал нь `range_capability`, `stream_budget`, болон
  `transport_hints` мета өгөгдөл. `/v1/sorafs/providers` хариултаар баталгаажуулна уу
  элсэлтийн хэрэгсэл; JSON хураангуй нь задлан шинжилсэн чадвар,
  урсгалын төсөв, телеметрийн хэрэглээнд зориулсан зөвлөмжийн массив.
- `cargo xtask sorafs-admission-fixtures` урсгал төсөв болон тээврийн гадаргуу
  JSON олдворууд доторх зөвлөмжүүд байдаг тул хяналтын самбар нь функцийг нэвтрүүлэхийг хянадаг.
- `fixtures/sorafs_manifest/provider_admission/`-ийн дагуу байрлах бэхэлгээнд одоо дараахь зүйлс орно.
  - каноник олон эх сурвалжтай зар сурталчилгаа,
  - `multi_fetch_plan.json`, ингэснээр SDK иж бүрдэл нь тодорхойлогч олон үе шатыг дахин тоглуулах боломжтой.
    төлөвлөгөө татах.

## Оркестратор ба Torii нэгдэл

- Torii `/v1/sorafs/providers` нь задалсан хүрээний чадамжийн мета өгөгдлийг буцаана
  `stream_budget` болон `transport_hints`-тай. Дахин зэрэглэлийг бууруулах анхааруулга гарах үед
  Үйлчилгээ үзүүлэгчид шинэ мета өгөгдлийг орхигдуулдаг бөгөөд гарцын хүрээний төгсгөлийн цэгүүд үүнийг хэрэгжүүлдэг
  шууд үйлчлүүлэгчдэд тавих хязгаарлалт.
- Олон эх сурвалжийн найруулагч (`sorafs_car::multi_fetch`) одоо хүрээг хэрэгжүүлж байна
  хязгаар, чадавхийг уялдуулах, ажил хуваарилахдаа урсгал төсөв. Нэгж
  Туршилтууд нь хэтэрхий том хэмжээтэй, сийрэг хайлт, тохируулагч хувилбаруудыг хамардаг.
- `sorafs_car::multi_fetch` зэрэглэлийг бууруулах дохиог дамжуулдаг (тохируулгын алдаа,
  хязгаарлагдмал хүсэлт) тул операторууд яагаад тодорхой үйлчилгээ үзүүлэгчид байсныг олж мэдэх боломжтой
  төлөвлөлтийн явцад алгассан.

## Телеметрийн лавлагаа

Torii хүрээний татах хэрэгсэл нь **SoraFS Татаж авах ажиглалт**-ыг хангадаг.
Grafana хяналтын самбар (`dashboards/grafana/sorafs_fetch_observability.json`) ба
дохиоллын хос дүрмүүд (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрик | Төрөл | Шошго | Тодорхойлолт |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | хэмжигч | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, I18NI0000 |0070X) Үйлчилгээ үзүүлэгчдийн сурталчилгааны хүрээний чадамжийн онцлогууд. |
| `torii_sorafs_range_fetch_throttle_events_total` | Тоолуур | `reason` (`quota`, `concurrency`, `byte_rate`) | Бодлогын дагуу бүлэглэсэн хязгаарлагдмал хүрээ татах оролдлого. |
| `torii_sorafs_range_fetch_concurrency_current` | хэмжигч | — | Хуваалцсан төсвийг зарцуулдаг идэвхтэй хамгаалагдсан урсгалууд. |

Жишээ PromQL хэсгүүд:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Идэвхжүүлэхээс өмнө тохируулагч тоолуурыг ашиглан квотын хэрэгжилтийг баталгаажуулна уу
олон эх сурвалжийн найруулагчийн өгөгдмөл тохируулгууд болон зэрэгцэн ирэх үед сэрэмжлүүлэх
өөрийн флотын төсвийн дээд хэмжээг дамжуулах.