---
lang: hy
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Data-Availability Threat-Model Automation (DA-1)

Ճանապարհային քարտեզի DA-1 կետը և `status.md`-ը պահանջում են դետերմինիստական ավտոմատացման հանգույց, որը
արտադրում է Norito PDP/PoTR սպառնալիքի մոդելի ամփոփագրերը, որոնք հայտնվել են
`docs/source/da/threat_model.md` և Docusaurus հայելին: Այս գրացուցակը
գրավում է արտեֆակտները, որոնք վկայակոչում են.

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (որն աշխատում է `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Հոսք

1. **Ստեղծեք հաշվետվություն**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON-ի ամփոփագիրը գրանցում է կրկնօրինակված ձախողման արագությունը, chunker-ը
   շեմերը և քաղաքականության ցանկացած խախտում, որը հայտնաբերված է PDP/PoTR սարքի կողմից
   `integration_tests/src/da/pdp_potr.rs`.
2. **Պատկերացրեք Markdown աղյուսակները**
   ```bash
   make docs-da-threat-model
   ```
   Սա գործարկում է `scripts/docs/render_da_threat_model_tables.py`՝ վերաշարադրելու համար
   `docs/source/da/threat_model.md` և `docs/portal/docs/da/threat-model.md`:
3. **Արխիվացրեք արտեֆակտը**՝ պատճենելով JSON զեկույցը (և կամընտիր CLI մատյանը)
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Երբ
   կառավարման որոշումները հիմնվում են կոնկրետ գործարկման վրա, ներառում են git commit hash-ը և
   սիմուլյատորի սերմ `<timestamp>-metadata.md` եղբոր մեջ:

## Ապացույցների ակնկալիքներ

- JSON ֆայլերը պետք է մնան <100 ԿԲ, որպեսզի կարողանան ապրել git-ում: Ավելի մեծ կատարում
  հետքերը պատկանում են արտաքին պահոցին՝ հղում կատարելով դրանց ստորագրված հեշին մետատվյալներում
  անհրաժեշտության դեպքում նշեք.
- Յուրաքանչյուր արխիվացված ֆայլ պետք է նշի սերմերը, կազմաձևման ուղին և սիմուլյատորի տարբերակը
  կրկնությունները կարող են վերարտադրվել հենց այն ժամանակ, երբ ստուգում են DA-ի թողարկման դարպասները:
- Հղում արխիվացված ֆայլին `status.md`-ից կամ ճանապարհային քարտեզի մուտքագրում, երբ
  DA-1-ի ընդունման չափանիշները առաջ են գնում՝ ապահովելով, որ վերանայողները կարող են ստուգել այն
  ելակետային գիծ՝ առանց ամրագոտու վերագործարկման:

## Պարտավորությունների հաշտեցում (Հաջորդականության բացթողում)

Օգտագործեք `cargo xtask da-commitment-reconcile`՝ DA մուտքագրման անդորրագրերը համեմատելու համար
DA-ի պարտավորությունների գրառումները, հաջորդականության հայտնաբերման բացթողումը կամ կեղծումը.

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Ընդունում է Norito կամ JSON ձևով անդորրագրեր և պարտավորություններ
  `SignedBlockWire`, `.norito` կամ JSON փաթեթներ:
- Չհաջողվեց, երբ որևէ տոմս բացակայում է բլոկի մատյանից կամ երբ հեշերը տարբերվում են.
  `--allow-unexpected`-ն անտեսում է միայն արգելափակման տոմսերը, երբ դուք միտումնավոր եք
  անդորրագրի հավաքածուն.
- Կցեք թողարկված JSON-ը կառավարման փաթեթներին/Alertmanager-ին բացթողման համար
  ահազանգեր; կանխադրված է `artifacts/da/commitment_reconciliation.json`:

## Արտոնությունների աուդիտ (Մուտքի եռամսյակային ակնարկ)

Օգտագործեք `cargo xtask da-privilege-audit`՝ սկանավորելու DA մանիֆեստի/կրկնակի գրացուցակները
(գումարած կամընտիր լրացուցիչ ուղիներ) բացակայող, ոչ գրացուցակի կամ աշխարհագրելի համար
գրառումներ:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Կարդում է DA մուտքագրման ուղիները տրամադրված Torii կոնֆիգուրից և ստուգում Unix-ը
  թույլտվությունները, որտեղ առկա են:
- Նշում է բացակայող/ոչ-տեղեկատու/աշխարհում գրվող ուղիները և վերադարձնում է ոչ զրոյական ելք
  կոդ, երբ խնդիրներ կան:
- Ստորագրեք և կցեք JSON փաթեթը (`artifacts/da/privilege_audit.json` by
  լռելյայն) եռամսյակային մուտքի և վերանայման փաթեթների և վահանակների համար: