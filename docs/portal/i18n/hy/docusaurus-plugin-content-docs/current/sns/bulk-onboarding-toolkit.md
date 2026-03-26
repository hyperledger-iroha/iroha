---
lang: hy
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
վերնագիր՝ SNS Bulk Onboarding Toolkit
sidebar_label. Զանգվածային միացման գործիքակազմ
նկարագրություն. CSV դեպի RegisterNameRequestV1 ավտոմատացում SN-3b ռեգիստրի գործարկումների համար:
---

:::note Կանոնական աղբյուր
Հայելիներ `docs/source/sns/bulk_onboarding_toolkit.md`, որպեսզի արտաքին օպերատորները տեսնեն
նույն SN-3b ուղեցույցը՝ առանց պահեստի կլոնավորման:
:::

# SNS զանգվածային տեղակայման գործիքակազմ (SN-3b)

**Ճանապարհային քարտեզի տեղեկանք.** SN-3b «Bulk onboarding tooling»  
**Արտեֆակտներ՝** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Խոշոր գրանցողները հաճախ նախապատմում են հարյուրավոր `.sora` կամ `.nexus` գրանցումներ
նույն կառավարման հաստատումներով և բնակավայրերի ռելսերով: JSON-ի ձեռքով պատրաստում
ծանրաբեռնվածությունը կամ CLI-ի վերագործարկումը չի մասշտաբվում, ուստի SN-3b-ն առաքում է որոշիչ
CSV-ից մինչև Norito շինարար, որը պատրաստում է `RegisterNameRequestV1` կառուցվածքները
Torii կամ CLI: Օգնականը հաստատում է յուրաքանչյուր տող առջևից, արձակում է երկուսն էլ
համախմբված մանիֆեստը և կամընտիր նոր տողով սահմանազատված JSON, և կարող է ներկայացնել
բեռը ավտոմատ կերպով կատարվում է աուդիտի համար կառուցվածքային մուտքերը գրանցելիս:

## 1. CSV սխեմա

Վերլուծիչը պահանջում է հետևյալ վերնագրի տողը (պատվերը ճկուն է).

| Սյունակ | Պահանջվում է | Նկարագրություն |
|--------|----------|-------------|
| `label` | Այո | Հայցված պիտակ (ընդունված է խառը գործը. գործիքը նորմալացվում է ըստ Նորմ v1 և UTS-46): |
| `suffix_id` | Այո | Թվային վերջածանցի նույնացուցիչ (տասնորդական կամ `0x` վեցանկյուն): |
| `owner` | Այո | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Այո | Ամբողջ թիվ `1..=255`. |
| `payment_asset_id` | Այո | Հաշվարկային ակտիվ (օրինակ՝ `61CtjvNd9T3THAR65GsMVHr82Bjc`): |
| `payment_gross` / `payment_net` | Այո | Աննշան ամբողջ թվեր, որոնք ներկայացնում են ակտիվների բնածին միավորները: |
| `settlement_tx` | Այո | JSON արժեք կամ բառացի տող, որը նկարագրում է վճարման գործարքը կամ հեշը: |
| `payment_payer` | Այո | Հաշվի ID, որը թույլ է տվել վճարումը: |
| `payment_signature` | Այո | JSON կամ բառացի տող, որը պարունակում է տնտեսվարի կամ գանձապետարանի ստորագրության ապացույց: |
| `controllers` | Ընտրովի | Ստորակետով կամ ստորակետով բաժանված վերահսկիչի հաշվի հասցեների ցանկը: Կանխադրված է `[owner]`, երբ բաց թողնված է: |
| `metadata` | Ընտրովի | Ներկառուցված JSON կամ `@path/to/file.json`, որոնք տրամադրում են լուծիչի հուշումներ, TXT գրառումներ և այլն: Կանխադրված է `{}`: |
| `governance` | Ընտրովի | Ներկառուցված JSON կամ `@path`՝ ուղղված դեպի `GovernanceHookV1`: `--require-governance`-ը պարտադրում է այս սյունակը: |

Ցանկացած սյունակ կարող է հղում կատարել արտաքին ֆայլի՝ նախածանցով բջջային արժեքը `@`-ով:
Ճանապարհները լուծվում են CSV ֆայլի համեմատ:

## 2. Վազում օգնականին

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Հիմնական ընտրանքներ.

- `--require-governance`-ը մերժում է առանց կառավարման կեռիկի տողերը (օգտակար է
  պրեմիում աճուրդներ կամ վերապահված հանձնարարություններ):
- `--default-controllers {owner,none}`-ը որոշում է, թե արդյոք դատարկ են վերահսկիչի բջիջները
  վերադառնալ սեփականատիրոջ հաշվին:
- `--controllers-column`, `--metadata-column` և `--governance-column` վերանվանել
  կամընտիր սյունակներ, երբ աշխատում են արտահանման հետ:

Հաջողության մասին սցենարը գրում է համախմբված մանիֆեստ.

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
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

Եթե տրամադրվում է `--ndjson`, ապա յուրաքանչյուր `RegisterNameRequestV1` գրվում է նաև որպես
մեկ տողով JSON փաստաթուղթ, որպեսզի ավտոմատացումները կարողանան ուղղակիորեն ուղարկել հարցումները
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Ավտոմատացված ներկայացումներ

### 3.1 Torii ՀԱՆԳՍՏԻ ռեժիմ

Նշեք `--submit-torii-url` գումարած կամ `--submit-token` կամ
`--submit-token-file` յուրաքանչյուր մանիֆեստի մուտքագրում ուղղակիորեն Torii մղելու համար.

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Օգնականը թողարկում է մեկ `POST /v1/sns/names` մեկ հարցում և ընդհատում է
  առաջին HTTP սխալը. Պատասխանները կցվում են գրանցամատյանում որպես NDJSON
  գրառումներ.
- `--poll-status` կրկին հարցում է անում `/v1/sns/names/{namespace}/{literal}` յուրաքանչյուրից հետո
  ներկայացում (մինչև `--poll-attempts`, լռելյայն 5)՝ հաստատելու, որ գրառումը
  տեսանելի. Տրամադրեք `--suffix-map` (JSON-ից `suffix_id`-ից մինչև `"suffix"` արժեքներ), որպեսզի
  Գործիքը կարող է ստանալ `{label}.{suffix}` բառացի քվեարկության համար:
- Կարգավորվողներ՝ `--submit-timeout`, `--poll-attempts` և `--poll-interval`:

### 3.2 iroha CLI ռեժիմ

Յուրաքանչյուր մանիֆեստի մուտքը CLI-ով ուղղորդելու համար տրամադրեք երկուական ուղին՝

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Կարգավորիչները պետք է լինեն `Account` գրառումներ (`controller_type.kind = "Account"`)
  քանի որ CLI-ն ներկայումս բացահայտում է միայն հաշվի վրա հիմնված վերահսկիչները:
- Մետատվյալները և կառավարման բլբերը գրվում են ժամանակավոր ֆայլերում յուրաքանչյուր հարցում և
  փոխանցվել է `iroha sns register --metadata-json ... --governance-json ...`:
- CLI stdout և stderr plus ելքի կոդերը գրանցված են. ոչ զրոյական ելքի կոդերն ընդհատվում են
  վազքը.

Ներկայացման երկու ռեժիմները կարող են աշխատել միասին (Torii և CLI)՝ գրանցողին խաչաձև ստուգելու համար
տեղակայումներ կամ հետադարձ փորձեր:

### 3.3 Ներկայացման անդորրագրեր

Երբ `--submission-log <path>` տրամադրվում է, սցենարը կցում է NDJSON գրառումները
գրավում:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Torii-ի հաջողված պատասխանները ներառում են կառուցվածքային դաշտեր՝ արդյունահանված
`NameRecordV1` կամ `RegisterNameResponseV1` (օրինակ, `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`), ուստի վահանակներ և կառավարում
հաշվետվությունները կարող են վերլուծել գրանցամատյանը՝ առանց ազատ ձևի տեքստի ստուգման: Կցեք այս գրանցամատյանը
գրանցման տոմսերը մանիֆեստի կողքին՝ վերարտադրելի ապացույցների համար:

## 4. Փաստաթղթերի պորտալի թողարկման ավտոմատացում

CI և պորտալի աշխատատեղերը զանգահարում են `docs/portal/scripts/sns_bulk_release.sh`, որը փաթաթվում է
օգնականը և պահպանում է արտեֆակտները `artifacts/sns/releases/<timestamp>/`-ի ներքո.

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

Սցենարը.

1. Կառուցում է `registrations.manifest.json`, `registrations.ndjson` և պատճենում
   բնօրինակ CSV-ն թողարկման գրացուցակում:
2. Ներկայացնում է մանիֆեստը՝ օգտագործելով Torii և/կամ CLI (երբ կազմաձևված է)՝ գրելով
   `submissions.log` վերը նշված կառուցվածքային անդորրագրերով:
3. Թողարկում է `summary.json`՝ նկարագրելով թողարկումը (ուղիներ, Torii URL, CLI ուղի,
   ժամանակի դրոշմ), այնպես որ պորտալի ավտոմատացումը կարող է փաթեթը վերբեռնել արտեֆակտի պահեստում:
4. Արտադրում է `metrics.prom` (գերակայել `--metrics`-ի միջոցով) պարունակող
   Prometheus ֆորմատի հաշվիչներ ընդհանուր հարցումների համար, վերջածանցների բաշխում,
   ակտիվների ընդհանուր գումարները և ներկայացման արդյունքները: Համառոտ JSON-ը հղում է այս ֆայլին:

Աշխատանքային հոսքերը պարզապես արխիվացնում են թողարկման գրացուցակը որպես մեկ արտեֆակտ, որն այժմ
պարունակում է այն ամենը, ինչ անհրաժեշտ է կառավարման համար աուդիտի համար:

## 5. Հեռաչափություն և վահանակներ

`sns_bulk_release.sh`-ի կողմից ստեղծված չափման ֆայլը բացահայտում է հետևյալը
շարք:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Սնուցեք `metrics.prom` ձեր Prometheus կողային մեքենայի մեջ (օրինակ՝ Promtail-ի կամ
խմբաքանակի ներմուծող) գրանցողներին, ստյուարդներին և կառավարման գործընկերներին համապատասխանեցված պահելու համար
զանգվածային առաջընթաց: Grafana տախտակ
`dashboards/grafana/sns_bulk_release.json`-ը պատկերացնում է նույն տվյալները վահանակներով
յուրաքանչյուր վերջածանցի հաշվարկի, վճարման ծավալի և ներկայացման հաջողության/ձախողման հարաբերակցության համար:
Տախտակը զտվում է `release`-ով, որպեսզի աուդիտորները կարողանան փորել մեկ CSV գործարկում:

## 6. Վավերացման և ձախողման ռեժիմներ

- **Պիտակների կանոնականացում.** մուտքերը նորմալացված են Python IDNA plus-ի միջոցով
  փոքրատառ և Norm v1 նիշերի զտիչներ: Անվավեր պիտակները արագորեն ձախողվում են ցանկացածից առաջ
  ցանցային զանգեր.
- **Թվային պահակաձողեր.** վերջածանցների ID-ներ, ժամկետային տարիներ և գնային ակնարկներ պետք է ընկնեն
  `u16` և `u8` սահմաններում: Վճարման դաշտերը ընդունում են տասնորդական կամ վեցանկյուն ամբողջ թվեր
  մինչև `i64::MAX`:
- **Մետատվյալների կամ կառավարման վերլուծություն.** ներկառուցված JSON-ն ուղղակիորեն վերլուծվում է. ֆայլ
  հղումները լուծվում են CSV-ի գտնվելու վայրի համեմատ: Ոչ օբյեկտի մետատվյալներ
  առաջացնում է վավերացման սխալ:
- **Կարգավորիչներ.** դատարկ բջիջներ հարգում են `--default-controllers`: Տրամադրել բացահայտ
  վերահսկիչների ցուցակները (օրինակ՝ `soraカタカナ...;soraカタカナ...`) ոչ սեփականատիրոջը պատվիրելիս
  դերասաններ.

Անհաջողությունները հաղորդվում են համատեքստային տողերի համարներով (օրինակ
`error: row 12 term_years must be between 1 and 255`): Սցենարը դուրս է գալիս
կոդը `1` վավերացման սխալների դեպքում և `2`, երբ CSV ուղին բացակայում է:

## 7. Փորձարկում և ծագում

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` ծածկում է CSV վերլուծությունը,
  NDJSON արտանետում, կառավարման կիրարկում և CLI կամ Torii ներկայացում
  ուղիները.
- Օգնականը մաքուր Python է (առանց լրացուցիչ կախվածության) և աշխատում է ցանկացած վայրում
  `python3` հասանելի է: Պարտավորությունների պատմությունը հետևվում է CLI-ի հետ մեկտեղ
  վերարտադրելիության հիմնական պահեստ:

Արտադրական գործարկումների համար կցեք ստեղծված մանիֆեստը և NDJSON փաթեթը
գրանցման տոմս, որպեսզի ստյուարդները կարողանան վերարտադրել ներկայացված ճշգրիտ բեռները
դեպի Torii: