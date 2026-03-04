---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bb0965d4125aa2c3a3d483b63f4b36b1c6bf26406a2fd54e645e7a3c0156c264
source_last_modified: "2026-01-05T09:28:11.906678+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Көпкөзді провайдердің жарнамалары және жоспарлау

Бұл бетте канондық спецификацияны ажыратады
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Бұл құжатты сөзбе-сөз Norito схемалары мен өзгерту журналдары үшін пайдаланыңыз; порталдың көшірмесі
оператор нұсқауларын, SDK жазбаларын және телеметрия сілтемелерін қалғандарына жақын сақтайды
SoraFS runbooks.

## Norito схема толықтырулары

### Ауқым мүмкіндігі (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – сұрау үшін ең үлкен іргелес аралық (байт), `≥ 1`.
- `min_granularity` – рұқсат іздеу, `1 ≤ value ≤ max_chunk_span`.
- `supports_sparse_offsets` – бір сұрауда іргелес емес ауыстыруларға рұқсат береді.
- `requires_alignment` – шын болғанда, ығысулар `min_granularity` сәйкес келуі керек.
- `supports_merkle_proof` – PoR куәгерінің қолдауын көрсетеді.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` канондық кодтауды енгізу
сондықтан өсек жүктемелері детерминистік болып қалады.

### `StreamBudgetV1`
- Өрістер: `max_in_flight`, `max_bytes_per_sec`, қосымша `burst_bytes`.
- Тексеру ережелері (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, бар болса, `> 0` және `≤ max_bytes_per_sec` болуы керек.

### `TransportHintV1`
- Өрістер: `protocol: TransportProtocol`, `priority: u8` (0–15 терезе орындалған
  `TransportHintV1::validate`).
- Белгілі протоколдар: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Әр провайдерге қайталанатын хаттама жазбалары қабылданбайды.

### `ProviderAdvertBodyV1` толықтырулар
- Қосымша `stream_budget: Option<StreamBudgetV1>`.
- Қосымша `transport_hints: Option<Vec<TransportHintV1>>`.
- Екі өріс енді `ProviderAdmissionProposalV1`, басқару арқылы өтеді
  конверттер, CLI құрылғылары және телеметриялық JSON.

## Валидация және басқару міндеттілігі

`ProviderAdvertBodyV1::validate` және `ProviderAdmissionProposalV1::validate`
дұрыс емес метадеректерден бас тарту:

- Ауқым мүмкіндіктері кодты шешуі және ауқым/түйірлік шектерін қанағаттандыруы керек.
- Ағын бюджеттері/тасымалдау туралы кеңестер сәйкестікті қажет етеді
  `CapabilityType::ChunkRangeFetch` TLV және бос емес кеңестер тізімі.
- Көшірме хаттамалары мен жарамсыз басымдықтар валидацияны арттырады
  жарнамалар өсек айтылмас бұрын қателер.
- Қабылдау конверттері диапазондағы метадеректерге арналған ұсыныстарды/жарнамаларды салыстырады
  `compare_core_fields` сондықтан сәйкес келмейтін өсек жүктемелері ерте қабылданбайды.

Регрессиялық қамту өмір сүреді
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Құралдар мен құрылғылар

- Провайдер жарнамасының пайдалы жүктемелері `range_capability`, `stream_budget` және
  `transport_hints` метадеректері. `/v1/sorafs/providers` жауаптары арқылы растаңыз және
  қабылдау құрылғылары; JSON қорытындылары талданған мүмкіндіктерді қамтуы керек,
  ағындық бюджет және телеметрияны қабылдауға арналған кеңес массивтері.
- `cargo xtask sorafs-admission-fixtures` ағындық бюджеттер мен көліктерді көрсетеді
  бақылау тақталары мүмкіндіктің қабылдануын бақылайтындай JSON артефактілеріндегі кеңестер.
- `fixtures/sorafs_manifest/provider_admission/` астында арматура енді мыналарды қамтиды:
  - канондық көп дереккөзді жарнамалар,
  - `multi_fetch_plan.json`, сондықтан SDK жиынтықтары детерминирленген мультипритті қайталай алады
    жоспарын алу.

## Оркестр және Torii интеграциясы

- Torii `/v1/sorafs/providers` талданған ауқым мүмкіндігінің метадеректерін қайтарады
  `stream_budget` және `transport_hints`. Төменгі деңгей туралы ескертулер пайда болған кезде
  провайдерлер жаңа метадеректерді өткізіп жібереді, ал шлюз ауқымының соңғы нүктелері бірдей күшіне енеді
  тікелей клиенттер үшін шектеулер.
- Көп көзді оркестр (`sorafs_car::multi_fetch`) енді диапазонды күшейтеді
  шектеулер, мүмкіндіктерді теңестіру және жұмысты тағайындау кезінде ағындық бюджеттер. Бірлік
  сынақтар тым үлкен, сирек іздеу және қысқарту сценарийлерін қамтиды.
- `sorafs_car::multi_fetch` төмендетілген сигналдар ағындары (туралау ақаулары,
  қысқартылған сұраулар) операторлар нақты провайдерлердің неліктен болғанын бақылай алады
  жоспарлау кезінде өткізіп жіберді.

## Телеметрия анықтамасы

Torii диапазонындағы алу аспаптары **SoraFS Fetch бақылау мүмкіндігін береді**
Grafana бақылау тақтасы (`dashboards/grafana/sorafs_fetch_observability.json`) және
жұптастырылған ескерту ережелері (`dashboards/alerts/sorafs_fetch_rules.yml`).

| метрикалық | |түрі Белгілер | Сипаттама |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Өлшеуіш | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, I18NI0000 |0070X) Провайдерлердің жарнамалық ауқым мүмкіндіктері мүмкіндіктері. |
| `torii_sorafs_range_fetch_throttle_events_total` | Есептегіш | `reason` (`quota`, `concurrency`, `byte_rate`) | Саясат бойынша топтастырылған қысқартылған ауқымды алу әрекеттері. |
| `torii_sorafs_range_fetch_concurrency_current` | Өлшеуіш | — | Ортақ параллельдік бюджетті тұтынатын белсенді қорғалатын ағындар. |

PromQL үзінділерінің мысалы:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Қосар алдында квотаның орындалуын растау үшін дроссель есептегішті пайдаланыңыз
көп көзді оркестрдің әдепкі мәндері және параллельдік жақындаған кезде ескерту
флотыңыз үшін максималды ағындық бюджет.