---
lang: ka
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: bulk-onboarding-toolkit
title: SNS Bulk Onboarding Toolkit
sidebar_label: მასობრივი ჩართვის ხელსაწყოების ნაკრები
აღწერა: CSV RegisterNameRequestV1 ავტომატიზაციისთვის SN-3b რეგისტრატორის გაშვებისთვის.
---

:::შენიშვნა კანონიკური წყარო
სარკეები `docs/source/sns/bulk_onboarding_toolkit.md` ასე რომ გარე ოპერატორები ხედავენ
იგივე SN-3b სახელმძღვანელო საცავი კლონირების გარეშე.
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**საგზაო რუკის მითითება:** SN-3b "Bulk onboarding tooling"  
**არტეფაქტები:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

დიდი რეგისტრატორები ხშირად წინასწარ ატარებენ ასობით `.sora` ან `.nexus` რეგისტრაციას
იგივე მმართველობის დამტკიცებითა და დასახლების რელსებით. JSON-ის ხელით დამზადება
ტვირთამწეობა ან CLI-ის ხელახლა გაშვება არ მასშტაბირებს, ამიტომ SN-3b აგზავნის დეტერმინისტულ
CSV to Norito მშენებელი, რომელიც ამზადებს `RegisterNameRequestV1` სტრუქტურებს
Torii ან CLI. დამხმარე ამოწმებს ყველა მწკრივს წინ, ასხივებს ორივეს ან
აგრეგირებული მანიფესტი და სურვილისამებრ ახალი ხაზით გამოყოფილი JSON და შეუძლია გაგზავნოს
იტვირთება ავტომატურად აუდიტის სტრუქტურირებული ქვითრების ჩაწერისას.

## 1. CSV სქემა

პარსერს სჭირდება შემდეგი სათაურის სტრიქონი (შეკვეთა მოქნილია):

| სვეტი | საჭირო | აღწერა |
|--------|----------|-------------|
| `label` | დიახ | მოთხოვნილი ეტიკეტი (მიღებულია შერეული შემთხვევა; ინსტრუმენტი ნორმალიზდება ნორმის მიხედვით v1 და UTS-46). |
| `suffix_id` | დიახ | რიცხვითი სუფიქსის იდენტიფიკატორი (ათწილადი ან `0x` თექვსმეტობითი). |
| `owner` | დიახ | AccountId სტრიქონი (IH58 ლიტერალური; სურვილისამებრ @domain მინიშნება) რეგისტრაციის მფლობელისთვის. |
| `term_years` | დიახ | მთელი რიცხვი `1..=255`. |
| `payment_asset_id` | დიახ | ანგარიშსწორების აქტივი (მაგალითად `xor#sora`). |
| `payment_gross` / `payment_net` | დიახ | ხელმოუწერელი მთელი რიცხვები, რომლებიც წარმოადგენენ აქტივის მშობლიურ ერთეულებს. |
| `settlement_tx` | დიახ | JSON მნიშვნელობა ან პირდაპირი სტრიქონი, რომელიც აღწერს გადახდის ტრანზაქციას ან ჰეშს. |
| `payment_payer` | დიახ | ანგარიშის ID, რომელიც აძლევდა უფლებას გადახდას. |
| `payment_signature` | დიახ | JSON ან პირდაპირი სტრიქონი, რომელიც შეიცავს სტიუარდის ან ხაზინის ხელმოწერის მტკიცებულებას. |
| `controllers` | სურვილისამებრ | მძიმით ან მძიმით გამოყოფილი კონტროლერის ანგარიშის მისამართების სია. ნაგულისხმევად არის `[owner]`, როდესაც გამოტოვებულია. |
| `metadata` | სურვილისამებრ | Inline JSON ან `@path/to/file.json` უზრუნველყოფს გადამწყვეტის მინიშნებებს, TXT ჩანაწერებს და ა.შ. ნაგულისხმევად არის `{}`. |
| `governance` | სურვილისამებრ | Inline JSON ან `@path`, რომელიც მიუთითებს `GovernanceHookV1`-ზე. `--require-governance` ახორციელებს ამ სვეტს. |

ნებისმიერ სვეტს შეუძლია მიუთითოს გარე ფაილი უჯრედის მნიშვნელობის პრეფიქსით `@`-ით.
ბილიკები წყდება CSV ფაილთან შედარებით.

## 2. დამხმარის გაშვება

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

ძირითადი პარამეტრები:

- `--require-governance` უარყოფს სტრიქონებს მმართველობის კვალის გარეშე (გამოსადგება
  პრემიუმ აუქციონები ან დაჯავშნილი დავალებები).
- `--default-controllers {owner,none}` წყვეტს, ცარიელია თუ არა კონტროლერის უჯრედები
  დაბრუნდეს მფლობელის ანგარიშზე.
- `--controllers-column`, `--metadata-column` და `--governance-column` გადარქმევა
  არასავალდებულო სვეტები ზემოთ ექსპორტთან მუშაობისას.

წარმატების შემთხვევაში სცენარი წერს აგრეგირებულ მანიფესტს:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "ih58...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"ih58...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"ih58...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

თუ მოწოდებულია `--ndjson`, თითოეული `RegisterNameRequestV1` ასევე იწერება როგორც
ერთსტრიქონიანი JSON დოკუმენტი, რათა ავტომატიზაციამ შეძლოს მოთხოვნების პირდაპირ გადაცემა
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. ავტომატური წარდგენები

### 3.1 Torii დასვენების რეჟიმი

მიუთითეთ `--submit-torii-url` პლუს `--submit-token` ან
`--submit-token-file` ყოველი მანიფესტის ჩანაწერის გადასატანად პირდაპირ Torii-ში:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- დამხმარე გასცემს ერთ `POST /v1/sns/registrations` მოთხოვნას და წყვეტს
  პირველი HTTP შეცდომა. პასუხები დართულია ჟურნალის ბილიკზე, როგორც NDJSON
  ჩანაწერები.
- `--poll-status` ხელახლა სვამს შეკითხვას `/v1/sns/registrations/{selector}` ყოველი შემდეგ
  წარდგენა (`--poll-attempts`-მდე, ნაგულისხმევი 5) დასადასტურებლად, რომ ჩანაწერი არის
  ხილული. მიუთითეთ `--suffix-map` (JSON of `suffix_id` to `"suffix"` მნიშვნელობები) ასე
  ხელსაწყოს შეუძლია გამოიტანოს `{label}.{suffix}` ლიტერალები გამოკითხვისთვის.
- მორგება: `--submit-timeout`, `--poll-attempts` და `--poll-interval`.

### 3.2 iroha CLI რეჟიმი

თითოეული მანიფესტის ჩანაწერის CLI-ში გადასატანად, მიაწოდეთ ბინარული გზა:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- კონტროლერები უნდა იყოს `Account` ჩანაწერები (`controller_type.kind = "Account"`)
  რადგან CLI ამჟამად ავლენს მხოლოდ ანგარიშზე დაფუძნებულ კონტროლერებს.
- მეტამონაცემები და მმართველობის ბლოგები იწერება დროებით ფაილებზე თითო მოთხოვნით და
  გადაგზავნილია `iroha sns register --metadata-json ... --governance-json ...`-ზე.
- ჩაწერილია CLI stdout და stderr plus გასასვლელი კოდები; არანულოვანი გასასვლელი კოდები წყვეტს
  გაშვება.

წარდგენის ორივე რეჟიმი შეიძლება ერთად მუშაობდეს (Torii და CLI) რეგისტრატორის გადასამოწმებლად
განლაგება ან რეპეტიციები.

### 3.3 წარდგენის ქვითრები

როდესაც მოწოდებულია `--submission-log <path>`, სკრიპტი აერთებს NDJSON ჩანაწერებს
გადაღება:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

წარმატებული Torii პასუხები მოიცავს სტრუქტურირებულ ველებს, რომლებიც ამოღებულია
`NameRecordV1` ან `RegisterNameResponseV1` (მაგალითად `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) ასე რომ, დაფები და მართვა
ანგარიშებს შეუძლიათ ჟურნალის გაანალიზება თავისუფალი ფორმის ტექსტის შემოწმების გარეშე. მიამაგრეთ ეს ჟურნალი
რეგისტრატორის ბილეთები მანიფესტთან ერთად გამეორებადი მტკიცებულებისთვის.

## 4. Docs პორტალის გამოშვების ავტომატიზაცია

CI და პორტალის სამუშაოები დარეკეთ `docs/portal/scripts/sns_bulk_release.sh`, რომელიც შეფუთულია
დამხმარე და ინახავს არტეფაქტებს `artifacts/sns/releases/<timestamp>/` ქვეშ:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

სცენარი:

1. აშენებს `registrations.manifest.json`, `registrations.ndjson` და აკოპირებს
   ორიგინალი CSV გამოშვების დირექტორიაში.
2. აგზავნის მანიფესტს Torii-ის და/ან CLI-ის (კონფიგურაციისას) გამოყენებით, წერით
   `submissions.log` სტრუქტურირებული ქვითრებით ზემოთ.
3. გამოსცემს `summary.json`, რომელიც აღწერს გამოშვებას (ბილიკები, Torii URL, CLI ბილიკი,
   დროის შტამპი) ასე რომ, პორტალის ავტომატიზაციას შეუძლია ატვირთოს ნაკრები არტეფაქტის საცავში.
4. აწარმოებს `metrics.prom` (გადალახვა `--metrics`-ით) შემცველი
   Prometheus ფორმატის მრიცხველები მთლიანი მოთხოვნებისთვის, სუფიქსის განაწილება,
   აქტივების ჯამი და წარდგენის შედეგები. შემაჯამებელი JSON ბმულებია ამ ფაილთან.

Workflows უბრალოდ დაარქივებს გამოშვების დირექტორიას, როგორც ერთ არტეფაქტს, რომელიც ახლა
შეიცავს ყველაფერს, რაც მმართველობას სჭირდება აუდიტისთვის.

## 5. ტელემეტრია და დაფები

`sns_bulk_release.sh`-ის მიერ გენერირებული მეტრიკის ფაილი ასახავს შემდეგს
სერია:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

შეიტანეთ `metrics.prom` თქვენს Prometheus გვერდით კარში (მაგალითად, Promtail-ის ან
პარტიული იმპორტიორი) რეგისტრატორების, სტიუარდების და მმართველობის თანატოლების თანხვედრის მიზნით
ნაყარი პროგრესი. Grafana დაფა
`dashboards/grafana/sns_bulk_release.json` ვიზუალიზაციას უკეთებს იმავე მონაცემებს პანელებით
თითო სუფიქსის დათვლისთვის, გადახდის მოცულობისა და წარდგენის წარმატება/წარუმატებლობის კოეფიციენტებისთვის.
დაფა ფილტრავს `release`-ით, რათა აუდიტორებმა შეძლონ ერთჯერადი CSV გაშვება.

## 6. ვალიდაციის და წარუმატებლობის რეჟიმები

- **ლეიბლის კანონიკიზაცია:** შეყვანები ნორმალიზებულია Python IDNA plus-ით
  მცირე და ნორმა v1 სიმბოლოების ფილტრები. არასწორი ეტიკეტები სწრაფად იშლება ნებისმიერზე ადრე
  ქსელური ზარები.
- ** რიცხვითი დამცავი ღობეები: ** სუფიქსის ID, ვადის წლები და ფასების მინიშნებები უნდა დაეცეს
  `u16` და `u8` ფარგლებში. გადახდის ველები მიიღებენ ათობითი ან თექვსმეტობით რიცხვებს
  `i64::MAX`-მდე.
- **მეტამონაცემების ან მმართველობის გარჩევა:** inline JSON ანალიზდება პირდაპირ; ფაილი
  მითითებები წყდება CSV მდებარეობის მიმართ. არაობიექტური მეტამონაცემები
  წარმოქმნის ვალიდაციის შეცდომას.
- **კონტროლერები:** ცარიელი უჯრედები პატივია `--default-controllers`. მიაწოდეთ მკაფიო
  კონტროლერების სიები (მაგალითად `ih58...;ih58...`) არამფლობელზე დელეგირებისას
  მსახიობები.

წარუმატებლობა მოხსენებულია კონტექსტური მწკრივის ნომრებით (მაგალითად
`error: row 12 term_years must be between 1 and 255`). სკრიპტი გადის
კოდი `1` ვალიდაციის შეცდომებზე და `2` როცა CSV გზა აკლია.

## 7. ტესტირება და წარმოშობა

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` მოიცავს CSV ანალიზს,
  NDJSON ემისია, მმართველობის აღსრულება და CLI ან Torii წარდგენა
  ბილიკები.
- დამხმარე არის სუფთა პითონი (დამატებითი დამოკიდებულების გარეშე) და მუშაობს სადმე
  `python3` ხელმისაწვდომია. ჩადენის ისტორიას თვალყურს ადევნებენ CLI-სთან ერთად
  განმეორებადობის მთავარი საცავი.

საწარმოო გაშვებებისთვის, მიამაგრეთ გენერირებული manifest და NDJSON პაკეტი
რეგისტრატორის ბილეთი, რათა სტიუარდებმა შეძლონ გაიმეორონ გაგზავნილი ზუსტი დატვირთვები
Torii-მდე.