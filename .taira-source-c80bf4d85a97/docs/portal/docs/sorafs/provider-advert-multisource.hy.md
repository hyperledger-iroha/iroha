---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bb0965d4125aa2c3a3d483b63f4b36b1c6bf26406a2fd54e645e7a3c0156c264
source_last_modified: "2026-01-05T09:28:11.906678+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Բազմաղբյուր մատակարարի գովազդ և պլանավորում

Այս էջը թորում է կանոնական բնութագրերը
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md):
Օգտագործեք այդ փաստաթուղթը բառացի Norito սխեմաների և փոփոխության մատյանների համար; պորտալի պատճենը
պահպանում է օպերատորի ուղեցույցը, SDK նշումները և հեռաչափության հղումները մնացածին մոտ
SoraFS runbook-երից:

## Norito սխեմայի հավելումներ

### միջակայքի հնարավորություն (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – ամենամեծ հարակից տիրույթը (բայթ) յուրաքանչյուր հարցման համար, `≥ 1`:
- `min_granularity` – որոնել լուծում, `1 ≤ value ≤ max_chunk_span`:
- `supports_sparse_offsets` – թույլ է տալիս ոչ հարակից հաշվանցումներ մեկ հարցումով:
- `requires_alignment` – երբ ճիշտ է, օֆսեթները պետք է համապատասխանեն `min_granularity`-ին:
- `supports_merkle_proof` – ցույց է տալիս PoR վկաների աջակցությունը:

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` պարտադրում է կանոնական կոդավորումը
այնպես որ բամբասանքների ծանրաբեռնվածությունը մնում է դետերմինիստական:

### `StreamBudgetV1`
- Դաշտեր՝ `max_in_flight`, `max_bytes_per_sec`, կամընտիր `burst_bytes`:
- Վավերացման կանոններ (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, երբ առկա է, պետք է լինի `> 0` և `≤ max_bytes_per_sec`:

### `TransportHintV1`
- Դաշտեր՝ `protocol: TransportProtocol`, `priority: u8` (0–15 պատուհանի ուժով
  `TransportHintV1::validate`):
- Հայտնի արձանագրություններ՝ `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Արձանագրությունների կրկնօրինակ գրառումները յուրաքանչյուր մատակարարի համար մերժվում են:

### `ProviderAdvertBodyV1` հավելումներ
- Լրացուցիչ `stream_budget: Option<StreamBudgetV1>`:
- Լրացուցիչ `transport_hints: Option<Vec<TransportHintV1>>`:
- Երկու դաշտերն էլ այժմ հոսում են `ProviderAdmissionProposalV1`, կառավարման միջոցով
  ծրարներ, CLI հարմարանքներ և հեռաչափական JSON:

## Վավերացում և կառավարում պարտադիր է

`ProviderAdvertBodyV1::validate` և `ProviderAdmissionProposalV1::validate`
մերժել սխալ ձևավորված մետատվյալները.

- Շրջանակի հնարավորությունները պետք է վերծանեն և բավարարեն միջակայքի/հատկավորության սահմանները:
- Հոսքի բյուջեները / տրանսպորտային ակնարկները պահանջում են համապատասխանություն
  `CapabilityType::ChunkRangeFetch` TLV և ոչ դատարկ ակնարկների ցուցակ:
- Կրկնվող տրանսպորտային արձանագրությունները և անվավեր առաջնահերթությունները բարձրացնում են վավերացումը
  սխալներ, նախքան գովազդի բամբասանքը:
- Ընդունելության ծրարները համեմատում են տեսականու մետատվյալների առաջարկները/գովազդները
  `compare_core_fields` այնքան անհամապատասխան բամբասանքների բեռները վաղաժամ են մերժվում:

Ռեգրեսիայի ծածկույթն ապրում է
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Գործիքներ և հարմարանքներ

- Մատակարարի գովազդային բեռները պետք է ներառեն `range_capability`, `stream_budget` և
  `transport_hints` մետատվյալներ. Վավերացնել `/v1/sorafs/providers` պատասխանների միջոցով և
  ընդունելության հարմարանքներ; JSON ամփոփագրերը պետք է ներառեն վերլուծված հնարավորությունը,
  հոսքային բյուջե և ակնարկային զանգվածներ՝ հեռաչափության կլանման համար:
- `cargo xtask sorafs-admission-fixtures` մակերեսները հոսում են բյուջեները և տրանսպորտը
  հուշում է իր JSON արտեֆակտների ներսում, որպեսզի վահանակները հետևեն գործառույթների ընդունմանը:
- `fixtures/sorafs_manifest/provider_admission/`-ի տակ գտնվող հարմարանքները այժմ ներառում են.
  - կանոնական բազմաղբյուր գովազդներ,
  - `multi_fetch_plan.json`, որպեսզի SDK հավաքակազմերը կարողանան վերարտադրել որոշիչ բազմակողմանի
    վերցնել պլանը:

## Նվագախմբի և Torii ինտեգրում

- Torii `/v1/sorafs/providers`-ը վերադարձնում է վերլուծված միջակայքի հնարավորությունների մետատվյալները միասին
  `stream_budget` և `transport_hints` հետ: Վարկանիշի իջեցման նախազգուշացումները միանում են, երբ
  մատակարարները բաց են թողնում նոր մետատվյալները, և դարպասների տիրույթի վերջնակետերը նույնն են պարտադրում
  սահմանափակումներ ուղղակի հաճախորդների համար:
- Բազմաղբյուր նվագախումբը (`sorafs_car::multi_fetch`) այժմ ապահովում է տիրույթը
  սահմանափակումներ, հնարավորությունների հավասարեցում և հոսքային բյուջեներ աշխատանք նշանակելիս: Միավոր
  թեստերն ընդգրկում են չափից ավելի մեծ, նոսր որոնումների և շնչափող սցենարներ:
- `sorafs_car::multi_fetch`-ը հոսում է իջեցման ազդանշաններ (հավասարեցման ձախողումներ,
  խափանված հարցումներ), որպեսզի օպերատորները կարողանան հետևել, թե ինչու են կոնկրետ պրովայդերները
  բաց է թողնվել պլանավորման ընթացքում:

## Հեռուստաչափության հղում

Torii միջակայքի առբերման գործիքավորումը ապահովում է **SoraFS Fetch Observability**
Grafana վահանակ (`dashboards/grafana/sorafs_fetch_observability.json`) և
Զուգակցված զգուշացման կանոններ (`dashboards/alerts/sorafs_fetch_rules.yml`):

| Մետրական | Տեսակ | Պիտակներ | Նկարագրություն |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Չափիչ | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Մատակարարների գովազդային տիրույթի հնարավորությունների առանձնահատկությունները: |
| `torii_sorafs_range_fetch_throttle_events_total` | Հաշվիչ | `reason` (`quota`, `concurrency`, `byte_rate`) | Կասեցված տիրույթի առբերման փորձերը խմբավորված ըստ քաղաքականության: |
| `torii_sorafs_range_fetch_concurrency_current` | Չափիչ | — | Ակտիվ պահպանվող հոսքեր, որոնք սպառում են ընդհանուր համաժամանակյա բյուջեն: |

PromQL հատվածների օրինակներ.

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Օգտագործեք շնչափող հաշվիչը՝ միացնելուց առաջ քվոտայի կիրառումը հաստատելու համար
բազմաղբյուր նվագախմբի լռակյաց կարգավորումներ և զգոն, երբ համաժամանակությունը մոտենում է
Ձեր նավատորմի համար բյուջեի առավելագույն հոսքը: