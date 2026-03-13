---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# მრავალ წყაროს პროვაიდერის რეკლამა და დაგეგმვა

ეს გვერდი ასახავს კანონიკურ სპეციფიკას
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
გამოიყენეთ ეს დოკუმენტი სიტყვასიტყვით Norito სქემებისა და ცვლილებების ჟურნალებისთვის; პორტალის ასლი
ინახავს ოპერატორის მითითებებს, SDK შენიშვნებს და ტელემეტრიის მითითებებს დანარჩენებთან ახლოს
SoraFS წიგნებიდან.

## Norito სქემის დამატებები

### დიაპაზონის შესაძლებლობა (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - ყველაზე დიდი მომიჯნავე დიაპაზონი (ბაიტი) თითო მოთხოვნაზე, `≥ 1`.
- `min_granularity` - გარჩევადობის ძიება, `1 ≤ value ≤ max_chunk_span`.
- `supports_sparse_offsets` - ნებადართულია არამიმღები ოფსეტები ერთ მოთხოვნაში.
- `requires_alignment` – როდესაც მართალია, ოფსეტები უნდა შეესაბამებოდეს `min_granularity`-ს.
- `supports_merkle_proof` – მიუთითებს PoR მოწმის მხარდაჭერაზე.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` აღასრულოს კანონიკური კოდირება
ასე რომ, ჭორის დატვირთვა დეტერმინისტული რჩება.

### `StreamBudgetV1`
- ველები: `max_in_flight`, `max_bytes_per_sec`, სურვილისამებრ `burst_bytes`.
- ვალიდაციის წესები (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, როდესაც არსებობს, უნდა იყოს `> 0` და `≤ max_bytes_per_sec`.

### `TransportHintV1`
- ველები: `protocol: TransportProtocol`, `priority: u8` (0–15 ფანჯარა იძულებით
  `TransportHintV1::validate`).
- ცნობილი პროტოკოლები: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- პროტოკოლის დუბლიკატი ჩანაწერები თითო პროვაიდერზე უარყოფილია.

### `ProviderAdvertBodyV1` დამატებები
- სურვილისამებრ `stream_budget: Option<StreamBudgetV1>`.
- სურვილისამებრ `transport_hints: Option<Vec<TransportHintV1>>`.
- ორივე ველი ახლა გადის `ProviderAdmissionProposalV1`, მმართველობით
  კონვერტები, CLI მოწყობილობები და ტელემეტრიული JSON.

## ვალიდაცია და მმართველობა სავალდებულოა

`ProviderAdvertBodyV1::validate` და `ProviderAdmissionProposalV1::validate`
არასწორი მეტამონაცემების უარყოფა:

- დიაპაზონის შესაძლებლობები უნდა გაშიფროს და აკმაყოფილებდეს სივრცის/გრანულარობის ლიმიტებს.
- ნაკადის ბიუჯეტები / ტრანსპორტის მინიშნებები მოითხოვს შესაბამისობას
  `CapabilityType::ChunkRangeFetch` TLV და არა ცარიელი მინიშნებების სია.
- სატრანსპორტო პროტოკოლების დუბლიკატი და არასწორი პრიორიტეტები ზრდის ვალიდაციას
  შეცდომები რეკლამების ჭორაობამდე.
- მისაღები კონვერტები ადარებენ წინადადებას/რეკლამებს დიაპაზონის მეტამონაცემებისთვის
  `compare_core_fields` ასე შეუსაბამო ჭორების დატვირთვა უარყოფილია ადრეულ პერიოდში.

რეგრესიის გაშუქება ცხოვრობს
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## ხელსაწყოები და მოწყობილობები

- პროვაიდერის რეკლამის დატვირთვა უნდა შეიცავდეს `range_capability`, `stream_budget` და
  `transport_hints` მეტამონაცემები. დადასტურება `/v2/sorafs/providers` პასუხებით და
  მისაღები მოწყობილობები; JSON შეჯამებები უნდა შეიცავდეს გაანალიზებულ შესაძლებლობებს,
  სტრიმინგის ბიუჯეტი და მინიშნებების მასივები ტელემეტრიის გადაყლაპვისთვის.
- `cargo xtask sorafs-admission-fixtures` ზედაპირების ნაკადის ბიუჯეტები და ტრანსპორტი
  მინიშნებები მისი JSON არტეფაქტების შიგნით, ასე რომ, დაფები თვალყურს ადევნებენ ფუნქციების მიღებას.
- `fixtures/sorafs_manifest/provider_admission/`-ის ქვეშ მყოფი მოწყობილობები ახლა მოიცავს:
  - კანონიკური მრავალ წყაროს რეკლამები,
  - `multi_fetch_plan.json`, რათა SDK კომპლექტებმა შეძლონ განმსაზღვრელი მრავალ თანატოლის გამეორება
    გეგმის მიღება.

## ორკესტრატორი და Torii ინტეგრაცია

- Torii `/v2/sorafs/providers` აბრუნებს გაანალიზებული დიაპაზონის შესაძლებლობების მეტამონაცემებს ერთად
  `stream_budget`-ით და `transport_hints`-ით. დაქვეითების გაფრთხილებები გაშვებულია, როდესაც
  პროვაიდერები გამოტოვებენ ახალ მეტამონაცემებს და კარიბჭის დიაპაზონის საბოლოო წერტილები იგივეს ახორციელებენ
  შეზღუდვები პირდაპირი კლიენტებისთვის.
- მრავალ წყაროს ორკესტრი (`sorafs_car::multi_fetch`) ახლა აძლიერებს დიაპაზონს
  სამუშაოს მინიჭებისას ლიმიტები, შესაძლებლობების გასწორება და ბიუჯეტების ნაკადი. ერთეული
  ტესტები მოიცავს ცალი ძალიან დიდ, მწირი ძიების და ჩახშობის სცენარებს.
- `sorafs_car::multi_fetch` ავრცელებს შემცირების სიგნალებს (გასწორების შეფერხებები,
  შემცირებული მოთხოვნები) ასე რომ, ოპერატორებს შეუძლიათ დაადგინონ, რატომ იყვნენ კონკრეტული პროვაიდერები
  დაგეგმვისას გამოტოვებული.

## ტელემეტრიის მითითება

Torii დიაპაზონის ჩამოტანის ინსტრუმენტაცია კვებავს **SoraFS Fetch Observability**
Grafana დაფა (`dashboards/grafana/sorafs_fetch_observability.json`) და
დაწყვილებული გაფრთხილების წესები (`dashboards/alerts/sorafs_fetch_rules.yml`).

| მეტრული | ტიპი | ეტიკეტები | აღწერა |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | ლიანდაგი | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | პროვაიდერები სარეკლამო დიაპაზონის შესაძლებლობების მახასიათებლები. |
| `torii_sorafs_range_fetch_throttle_events_total` | მრიცხველი | `reason` (`quota`, `concurrency`, `byte_rate`) | შემცირებული დიაპაზონის მოპოვების მცდელობები დაჯგუფებულია წესების მიხედვით. |
| `torii_sorafs_range_fetch_concurrency_current` | ლიანდაგი | — | აქტიური დაცული ნაკადები, რომლებიც მოიხმარენ საერთო კონკურენტულ ბიუჯეტს. |

PromQL ფრაგმენტების მაგალითი:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

გამოიყენეთ დროსელის მრიცხველი, რათა დაადასტუროთ კვოტის შესრულება ჩართვამდე
მრავალ წყაროს ორკესტრატორის ნაგულისხმევი პარამეტრები და გაფრთხილება, როდესაც კონკურენტულობა უახლოვდება
თქვენი ფლოტის საბიუჯეტო მაქსიმუმების ნაკადი.