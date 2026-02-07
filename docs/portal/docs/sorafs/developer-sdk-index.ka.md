---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28cc43e407412d66481f25146c19d35f0e102523d22f954be3c106231d95e891
source_last_modified: "2026-01-05T09:28:11.868629+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-index
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
---

:::შენიშვნა კანონიკური წყარო
:::

გამოიყენეთ ეს კერა, რათა თვალყური ადევნოთ თითოეულ ენაზე დამხმარეებს, რომლებიც იგზავნება SoraFS ხელსაწყოების ჯაჭვით.
Rust-ის სპეციფიკური ფრაგმენტებისთვის გადადით [Rust SDK snippets]-ზე (./developer-sdk-rust.md).

## ენის დამხმარეები

- **პითონი** - `sorafs_multi_fetch_local` (ადგილობრივი ორკესტრის კვამლის ტესტები) და
  `sorafs_gateway_fetch` (კარიბჭის E2E სავარჯიშოები) ახლა მიიღეთ სურვილისამებრ
  `telemetry_region` პლუს `transport_policy` გადაფარვა
  (`"soranet-first"`, `"soranet-strict"`, ან `"direct-only"`), ასახავს CLI-ს
  გაშვების სახელურები. როდესაც ადგილობრივი QUIC პროქსი ტრიალებს,
  `sorafs_gateway_fetch` აბრუნებს ბრაუზერის მანიფესტს ქვეშ
  `local_proxy_manifest`, რათა ტესტებმა შეძლოს ნდობის ნაკრები ბრაუზერის გადამყვანებს გადასცეს.
- ** JavaScript** — `sorafsMultiFetchLocal` ასახავს პითონის დამხმარეს, ბრუნდება
  დატვირთვის ბაიტები და ქვითრების შეჯამება, ხოლო `sorafsGatewayFetch` ვარჯიშობს
  Torii კარიბჭეები, threads ლოკალური პროქსი გამოიხატება და ავლენს იგივეს
  ტელემეტრია/ტრანსპორტი უგულებელყოფს როგორც CLI.
- **Rust** — სერვისებს შეუძლიათ განლაგების ჩაშენება პირდაპირ მეშვეობით
  `sorafs_car::multi_fetch`; იხილეთ [Rust SDK snippets] (./developer-sdk-rust.md)
  მითითება მტკიცებულებათა ნაკადის დამხმარეებისა და ორკესტრის ინტეგრაციისთვის.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` ხელახლა იყენებს Torii HTTP-ს
  შემსრულებელი და პატივს სცემს `GatewayFetchOptions`. შეუთავსეთ იგი
  `ClientConfig.Builder#setSorafsGatewayUri` და PQ ატვირთვის მინიშნება
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`), როდესაც ატვირთვები უნდა დარჩეს
  PQ მხოლოდ ბილიკები.

## შედეგების დაფა და პოლიტიკის ღილაკები

ორივე პითონი (`sorafs_multi_fetch_local`) და JavaScript
(`sorafsMultiFetchLocal`) დამხმარეები ამჟღავნებენ ტელემეტრიის მცოდნე გრაფიკის დაფას
გამოიყენება CLI-ს მიერ:

- საწარმოო ორობითი ფაილები ჩართავს ანგარიშების დაფას ნაგულისხმევად; კომპლექტი `use_scoreboard=True`
  (ან მიაწოდეთ `telemetry` ჩანაწერები) მოწყობილობების ხელახლა დაკვრისას, რათა დამხმარე გამოვიდეს
  შეწონილი პროვაიდერის შეკვეთა რეკლამის მეტამონაცემებიდან და ბოლო ტელემეტრიული კადრებიდან.
- დააყენეთ `return_scoreboard=True`, რომ მიიღოთ გამოთვლილი წონები ნაჭერთან ერთად
  ქვითრები, რათა CI ჟურნალებმა შეძლონ დიაგნოსტიკის აღბეჭდვა.
- გამოიყენეთ `deny_providers` ან `boost_providers` მასივები თანატოლების უარსაყოფად ან დასამატებლად
  `priority_delta` როდესაც დამგეგმავი ირჩევს პროვაიდერებს.
- შეინარჩუნეთ ნაგულისხმევი `"soranet-first"` პოზა, თუ არ განახორციელებთ დაქვეითებას; მიწოდება
  `"direct-only"` მხოლოდ მაშინ, როდესაც შესაბამისობის რეგიონმა თავიდან უნდა აიცილოს რელეები ან როდის
  SNNet-5a სარეზერვო რეპეტიცია და `"soranet-strict"` დაჯავშნა მხოლოდ PQ-სთვის
  პილოტები მმართველობის თანხმობით.
- კარიბჭის დამხმარეები ასევე ავლენენ `scoreboardOutPath` და `scoreboardNowUnixSecs`.
  დააყენეთ `scoreboardOutPath`, რათა შენარჩუნდეს გამოთვლილი ანგარიშების დაფა (ასახავს CLI-ს
  `--scoreboard-out` დროშა) ასე რომ, `cargo xtask sorafs-adoption-check` შეუძლია დაადასტუროს
  SDK არტეფაქტები და გამოიყენეთ `scoreboardNowUnixSecs`, როდესაც არმატურას სჭირდება სტაბილური
  `assume_now` მნიშვნელობა რეპროდუცირებადი მეტამონაცემებისთვის. JavaScript-ის დამხმარეში თქვენ
  შეუძლია დამატებით დააყენოს `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  როდესაც იარლიყი გამოტოვებულია, ის წარმოიქმნება `region:<telemetryRegion>` (უკან დაბრუნდება
  `sdk:js`-მდე). პითონის დამხმარე ავტომატურად გამოსცემს `telemetry_source="sdk:python"`
  ყოველთვის, როდესაც ის რჩება ანგარიშების დაფაზე და ინარჩუნებს იმპლიციტურ მეტამონაცემებს გამორთული.

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