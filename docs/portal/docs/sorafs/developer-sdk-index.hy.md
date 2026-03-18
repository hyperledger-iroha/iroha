---
lang: hy
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

:::note Կանոնական աղբյուր
:::

Օգտագործեք այս հանգույցը՝ SoraFS գործիքների շղթայով առաքվող յուրաքանչյուր լեզվով աշխատողներին հետևելու համար:
Rust-ին հատուկ հատվածների համար անցեք [Rust SDK snippets] (./developer-sdk-rust.md):

## Լեզվի օգնականներ

- **Python** — `sorafs_multi_fetch_local` (տեղական նվագախմբի ծխի թեստեր) և
  `sorafs_gateway_fetch` (gateway E2E վարժություններ) այժմ ընդունում է կամընտիր
  `telemetry_region` գումարած `transport_policy` փոխարինում
  (`"soranet-first"`, `"soranet-strict"`, կամ `"direct-only"`), արտացոլելով CLI-ը
  rollout բռնակներ. Երբ տեղական QUIC վստահված անձը պտտվում է,
  `sorafs_gateway_fetch`-ը վերադարձնում է դիտարկիչի մանիֆեստը տակ
  `local_proxy_manifest`, որպեսզի թեստերը կարողանան վստահության փաթեթը փոխանցել դիտարկիչի ադապտերներին:
- **JavaScript** — `sorafsMultiFetchLocal`-ը արտացոլում է Python-ի օգնականը՝ վերադառնալով
  ծանրաբեռնված բայթեր և անդորրագրերի ամփոփագրեր, մինչդեռ `sorafsGatewayFetch` վարժությունները
  Torii դարպասներ, թելեր տեղական վստահված անձը դրսևորվում է և բացահայտում նույնը
  Հեռաչափությունը/տրանսպորտը վերացնում է որպես CLI:
- **Rust** — ծառայությունները կարող են ներդնել ժամանակացույցը անմիջապես միջոցով
  `sorafs_car::multi_fetch`; տես [Rust SDK snippets] (./developer-sdk-rust.md)
  տեղեկանք ապացույցների հոսքի օգնականների և նվագախմբի ինտեգրման համար:
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)`-ը նորից օգտագործում է Torii HTTP-ը
  կատարող և մեծարում է `GatewayFetchOptions`. Միավորել այն
  `ClientConfig.Builder#setSorafsGatewayUri` և PQ վերբեռնման հուշում
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`), երբ վերբեռնումները պետք է պահպանվեն
  PQ-միայն ուղիներ:

## Ցուցատախտակ և քաղաքականության կոճակներ

Ինչպես Python-ը (`sorafs_multi_fetch_local`), այնպես էլ JavaScript-ը
(`sorafsMultiFetchLocal`) օգնականները բացահայտում են հեռաչափության տեղյակ ժամանակացույցի ցուցատախտակը
օգտագործված CLI-ի կողմից.

- Արտադրական երկուականները լռելյայն միացնում են ցուցատախտակը. հավաքածու `use_scoreboard=True`
  (կամ տրամադրեք `telemetry` գրառումներ) հարմարանքները վերախաղարկելիս, որպեսզի օգնականը ստանա
  կշռված մատակարարը, որը պատվիրում է գովազդի մետատվյալներից և վերջին հեռաչափական պատկերներից:
- Սահմանեք `return_scoreboard=True`՝ հաշվարկված կշիռները կտորի կողքին ստանալու համար
  անդորրագրեր, որպեսզի CI տեղեկամատյանները կարողանան ախտորոշել:
- Օգտագործեք `deny_providers` կամ `boost_providers` զանգվածներ՝ գործընկերներին մերժելու կամ ավելացնելու համար
  `priority_delta`, երբ ժամանակացույցն ընտրում է մատակարարներին:
- Պահպանեք լռելյայն `"soranet-first"` կեցվածքը, եթե չգնահատեք վարկանիշը; մատակարարում
  `"direct-only"` միայն այն դեպքում, երբ համապատասխանության շրջանը պետք է խուսափի ռելեներից կամ երբ
  կրկնում է SNNet-5a-ն և պահում `"soranet-strict"` միայն PQ-ի համար
  օդաչուներ՝ կառավարման հավանությամբ:
- Դարպասի օգնականները նաև բացահայտում են `scoreboardOutPath` և `scoreboardNowUnixSecs`:
  Սահմանեք `scoreboardOutPath`, որպեսզի պահպանվի հաշվարկված ցուցատախտակը (հայելում է CLI-ը
  `--scoreboard-out` դրոշ), այնպես որ `cargo xtask sorafs-adoption-check` կարող է վավերացնել
  SDK արտեֆակտներ և օգտագործեք `scoreboardNowUnixSecs`, երբ հարմարանքները կայուն կարիք ունեն
  `assume_now` արժեքը վերարտադրելի մետատվյալների համար: JavaScript-ի օգնականում դուք
  կարող է լրացուցիչ սահմանել `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  երբ պիտակը բաց է թողնվում, այն ստացվում է `region:<telemetryRegion>` (հետ գնալով
  դեպի `sdk:js`): Python օգնականը ավտոմատ կերպով թողարկում է `telemetry_source="sdk:python"`
  երբ այն պահպանում է արդյունքների տախտակը և անջատում է անուղղակի մետատվյալները:

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