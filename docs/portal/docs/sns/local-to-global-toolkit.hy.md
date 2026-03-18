---
lang: hy
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4493e69ce57c4f691f368fb13c1bbe96e2c73991dfb39045753b5652d2f10a9
source_last_modified: "2026-01-28T17:11:30.702818+00:00"
translation_last_reviewed: 2026-02-07
title: Local → Global Address Toolkit
translator: machine-google-reviewed
---

Այս էջը արտացոլում է [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
մոնո-ռեպոյից։ Այն փաթեթավորում է CLI-ի օգնականներն ու աշխատատեղերը, որոնք պահանջվում են ճանապարհային քարտեզի **ADDR-5c** կետով:

## Տեսություն

- `scripts/address_local_toolkit.sh`-ը փաթաթում է `iroha` CLI-ն՝ արտադրելու համար.
  - `audit.json` — կառուցվածքային ելք `iroha tools address audit --format json`-ից:
  - `normalized.txt` — փոխարկված նախընտրելի I105 / երկրորդ լավագույն սեղմված (`sora`) բառացիները տեղական տիրույթի յուրաքանչյուր ընտրողի համար:
- Զուգակցել սկրիպտը հասցեի մուտքագրման վահանակի հետ (`dashboards/grafana/address_ingest.json`)
  և Alertmanager կանոնները (`dashboards/alerts/address_ingest_rules.yml`)՝ տեղական-8-ն ապացուցելու համար
  Local-12 cutover-ը անվտանգ է: Դիտեք Local-8 և Local-12 բախման վահանակները, գումարած
  `AddressLocal8Resurgence`, `AddressLocal12Collision` և `AddressInvalidRatioSlo` նախազգուշացումներ
  ակնհայտ փոփոխությունների խթանում:
- Հղեք [Address Display Guidelines] (address-display-guidelines.md) և
  [Address Manifest runbook] (../../../source/runbooks/address_manifest_ops.md) UX-ի և միջադեպերի արձագանքման համատեքստի համար:

## Օգտագործում

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

Ընտրանքներ:

- `--format i105` `i105` ելքի համար՝ I105-ի փոխարեն:
- `domainless output (default)` բաց բառացի արտանետելու համար:
- `--audit-only`՝ փոխակերպման քայլը բաց թողնելու համար:
- `--allow-errors`՝ շարունակելու սկանավորումը, երբ սխալ տողեր են հայտնվում (համապատասխանում է CLI-ի վարքագծին):

Սցենարը գրում է արտեֆակտի ուղիները վազքի վերջում: Կցեք երկու ֆայլերը
ձեր փոփոխության կառավարման տոմսը Grafana սքրինշոթի կողքին, որն ապացուցում է զրո
Local-8 հայտնաբերումներ և զրոյական Local-12 բախումներ ≥30 օրվա ընթացքում:

## CI ինտեգրում

1. Գործարկեք սցենարը հատուկ աշխատանքով և վերբեռնեք դրա արդյունքները:
2. Արգելափակումը միաձուլվում է, երբ `audit.json` հաղորդում է Տեղական ընտրիչները (`domain.kind = local12`):
   իր լռելյայն `true` արժեքով (միայն փոխարինել `false`-ով մշակողների/փորձարկման կլաստերներում, երբ
   ռեգրեսիաների ախտորոշում) և ավելացնել
   `iroha tools address normalize` դեպի CI, ուստի ռեգրեսիա
   փորձերը ձախողվում են մինչև արտադրություն սկսելը:

Լրացուցիչ մանրամասների համար տե՛ս սկզբնաղբյուր փաստաթուղթը, օրինակելի ապացույցների ստուգաթերթերը և թողարկման նշումի հատվածը, որը կարող եք նորից օգտագործել՝ հաճախորդներին կտրվածքի մասին հայտարարելիս: