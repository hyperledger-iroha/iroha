---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Reserve+Rent քաղաքականությունը (ճանապարհային քարտեզի կետ **SFM‑6**) այժմ առաքում է `sorafs reserve`
CLI օգնականները գումարած `scripts/telemetry/reserve_ledger_digest.py` թարգմանիչը
գանձապետական վազքերը կարող են առաջացնել որոշիչ վարձավճար/պահուստային փոխանցումներ: Այս էջը հայելի է
`docs/source/sorafs_reserve_rent_plan.md`-ում սահմանված աշխատանքային հոսքը և բացատրում է
ինչպես միացնել փոխանցման նոր հոսքը Grafana + Alertmanager, որպեսզի տնտեսագիտություն և
Կառավարման վերանայողները կարող են աուդիտի ենթարկել վճարման յուրաքանչյուր ցիկլ:

## Աշխատանքի ավարտից մինչև վերջ

1. ** Մեջբերում + մատյանային պրոյեկցիա **
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Գրասենյակի օգնականը կցում է `ledger_projection` բլոկ (վարձավճար, պահուստ
   դեֆիցիտ, լիցքավորման դելտա, տեղաբաշխման բուլյաններ) գումարած Norito `Transfer`
   ISI-ներն անհրաժեշտ են XOR-ը գանձապետական և պահուստային հաշիվների միջև տեղափոխելու համար:

2. **Ստեղծեք digest + Prometheus/NDJSON ելքեր**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Դիջեստի օգնականը նորմալացնում է micro-XOR-ի գումարները XOR-ի, արձանագրում, թե արդյոք
   պրոյեկցիան համապատասխանում է տեղաբաշխմանը և թողարկում է **փոխանցման հոսք** չափումները
   `sorafs_reserve_ledger_transfer_xor` և
   `sorafs_reserve_ledger_instruction_total`. Երբ պետք է լինեն մի քանի մատյաններ
   մշակված (օրինակ՝ մատակարարների խմբաքանակ), կրկնել `--ledger`/`--label` զույգերը և
   օգնականը գրում է մեկ NDJSON/Prometheus ֆայլ, որը պարունակում է յուրաքանչյուր ամփոփում
   վահանակները կլանում են ամբողջ ցիկլը առանց պատվիրված սոսինձի: `--out-prom`
   ֆայլը ուղղված է հանգույց արտահանող տեքստային ֆայլերի հավաքագրողին. գցել `.prom` ֆայլը
   արտահանողի դիտված գրացուցակը կամ վերբեռնել այն հեռաչափության դույլում
   սպառվում է «Պահուստային վահանակի» աշխատանքի կողմից, մինչդեռ `--ndjson-out`-ը նույնն է սնվում
   ծանրաբեռնվածություն տվյալների խողովակաշարերում:

3. **Հրապարակեք արտեֆակտներ + ապացույցներ**
   - Պահեք մարսողությունները `artifacts/sorafs_reserve/ledger/<provider>/`-ի տակ և հղեք
     Markdown-ի ամփոփագիրը ձեր շաբաթական տնտեսագիտական զեկույցից:
   - Կցեք JSON digest-ը վարձավճարի այրմանը (որպեսզի աուդիտորները կարողանան կրկնել
     մաթեմատիկա) և ներառել ստուգիչ գումարը կառավարման ապացույցների փաթեթի ներսում:
   - Եթե digest-ը ազդարարում է լիցքավորման կամ տեղաբաշխման խախտման մասին, նշեք ահազանգը
     ID-ներ (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) և նշեք, թե որ փոխանցման ISI-ներն են եղել
     դիմել է.

## Չափումներ → վահանակներ → ահազանգեր

| Աղբյուրի մետրիկ | Grafana վահանակ | Զգուշացում / քաղաքականության կեռիկ | Ծանոթագրություններ |
|------------------------------------------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | «DA Rent Distribution (XOR/hour)» `dashboards/grafana/sorafs_capacity_health.json`-ում | Կերակրե՛ք գանձապետարանի շաբաթական դիջեստը; Պահուստային հոսքի հասկերը տարածվում են `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) մեջ: |
| `torii_da_rent_gib_months_total` | «Հզորությունների օգտագործում (GiB- ամիսներ)» (նույն վահանակ) | Զուգակցեք մատյանների ամփոփագրի հետ՝ ապացուցելու համար, որ հաշիվ-ապրանքագրված պահեստը համապատասխանում է XOR փոխանցումներին: |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | «Reserve Snapshot (XOR)» + կարգավիճակի քարտեր `dashboards/grafana/sorafs_reserve_economics.json`-ում | `SoraFSReserveLedgerTopUpRequired` կրակվում է, երբ `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` կրակվում է, երբ `meets_underwriting=0`: |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | «Տրանսֆերներ ըստ տեսակի», «Վերջին փոխանցումների բաշխում» և ծածկույթի քարտերը `dashboards/grafana/sorafs_reserve_economics.json`-ում | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` և `SoraFSReserveLedgerTopUpTransferMissing` նախազգուշացնում են, երբ փոխանցման հոսքը բացակայում է կամ զրոյացված է, թեև վարձակալություն/լրացում է պահանջվում. ծածկույթի քարտերը նույն դեպքերում ընկնում են մինչև 0%: |

Երբ վարձակալության ցիկլը ավարտվի, թարմացրեք Prometheus/NDJSON նկարները, հաստատեք
որ Grafana վահանակները վերցնում են նոր `label`-ը և կցում սքրինշոթեր +
Alertmanager ID-ներ վարձակալության կառավարման փաթեթին: Սա ապացուցում է CLI պրոյեկցիան,
հեռաչափությունը և կառավարման արտեֆակտները բոլորը բխում են **նույն** փոխանցման հոսքից և
պահում է ճանապարհային քարտեզի տնտեսական վահանակները՝ Reserve+Rent-ի հետ համահունչ
ավտոմատացում։ Ծածկույթի քարտերը պետք է կարդան 100% (կամ 1.0) և նոր ահազանգերը
պետք է մաքրվի, երբ վարձակալության և պահուստի համալրման փոխանցումները ներառվեն ամփոփագրում: